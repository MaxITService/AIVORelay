use crate::managers::model::{ModelInfo, ModelManager};
use crate::managers::transcription::{LoadingGuard, ModelStateEvent, TranscriptionManager};
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout, TranscriptionProvider};
use crate::tray;
use serde::Serialize;
use specta::Type;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

const MODEL_DELETION_DELAY: Duration = Duration::from_secs(5);

#[cfg(test)]
mod model_deletion_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Default, Clone)]
    struct Harness {
        state: Arc<ModelDeletionState>,
        events: Arc<Mutex<Vec<ModelDeletionSnapshot>>>,
        busy: Arc<AtomicBool>,
        deletions: Arc<AtomicUsize>,
    }

    struct Claim(Arc<AtomicBool>);
    impl Drop for Claim {
        fn drop(&mut self) { self.0.store(false, Ordering::SeqCst); }
    }

    type Worker = (String, Instant, tokio::sync::oneshot::Receiver<()>);

    impl Harness {
        fn publish(&self, snapshot: ModelDeletionSnapshot) {
            self.events.lock().unwrap().push(snapshot);
        }

        fn queue(&self, model: &str) -> (ModelDeletionSnapshot, Option<Worker>) {
            self.state.inner.lock().unwrap().schedule(model,
                || tokio::time::Instant::now().into_std(), || 1000, || Ok(()),
                &|snapshot| self.publish(snapshot)).unwrap()
        }

        fn cancel(&self, model: &str, job: &str) -> &'static str {
            self.state.inner.lock().unwrap().cancel_job(model, job, &|snapshot| self.publish(snapshot))
        }

        fn start(&self, model: &str, worker: Worker, release: Option<tokio::sync::oneshot::Receiver<()>>,
            fail: bool) -> tokio::task::JoinHandle<()> {
            let publisher = self.clone();
            let claimant = self.clone();
            let executor = self.clone();
            tokio::spawn(run_model_deletion(self.state.clone(), model.to_string(), worker.0,
                worker.1, worker.2, move |snapshot| publisher.publish(snapshot),
                move || claimant.busy.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .ok().map(|_| Claim(claimant.busy.clone())),
                move |claim| async move {
                    let _claim = claim;
                    // The executor only counts calls; it never touches model files or app resources.
                    executor.deletions.fetch_add(1, Ordering::SeqCst);
                    if let Some(release) = release { let _ = release.await; }
                    if fail { Err("disk failure".to_string()) } else { Ok(()) }
                }))
        }

        fn snapshot(&self) -> ModelDeletionSnapshot { self.state.inner.lock().unwrap().snapshot() }
        fn statuses(&self) -> Vec<String> {
            self.events.lock().unwrap().iter().map(|event| event.change.as_ref().unwrap().status.clone()).collect()
        }
    }

    async fn advance(ms: u64) {
        // Poll newly spawned workers before advancing, then poll expired timers.
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(ms)).await;
        for _ in 0..3 { tokio::task::yield_now().await; }
    }

    #[tokio::test(start_paused = true)]
    async fn destructive_executor_waits_for_the_full_undo_window() {
        let h = Harness::default();
        let (_, worker) = h.queue("a");
        let task = h.start("a", worker.unwrap(), None, false);
        advance(4999).await;
        assert_eq!(h.deletions.load(Ordering::SeqCst), 0);
        assert_eq!(h.snapshot().jobs[0].status, "pending");
        advance(1).await;
        assert_eq!(h.deletions.load(Ordering::SeqCst), 1);
        task.await.unwrap();
        assert_eq!(h.deletions.load(Ordering::SeqCst), 1);
        assert!(h.snapshot().jobs.is_empty());
        assert_eq!(h.statuses(), ["pending", "deleting", "completed"]);
    }

    #[tokio::test(start_paused = true)]
    async fn undo_before_deadline_prevents_deletion() {
        let h = Harness::default();
        let (snapshot, worker) = h.queue("a");
        let task = h.start("a", worker.unwrap(), None, false);
        advance(4999).await;
        assert_eq!(h.cancel("a", &snapshot.jobs[0].job_id), "cancelled");
        advance(5000).await;
        task.await.unwrap();
        assert_eq!(h.deletions.load(Ordering::SeqCst), 0);
        assert_eq!(h.statuses(), ["pending", "cancelled"]);
    }

    #[tokio::test(start_paused = true)]
    async fn selecting_a_pending_model_cancels_its_timer() {
        let h = Harness::default();
        let (_, worker) = h.queue("a");
        let task = h.start("a", worker.unwrap(), None, false);
        h.state.inner.lock().unwrap().cancel_for_use_with("a", &|snapshot| h.publish(snapshot)).unwrap();
        advance(5000).await;
        task.await.unwrap();
        assert_eq!(h.deletions.load(Ordering::SeqCst), 0);
        assert!(h.snapshot().jobs.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_request_keeps_one_job_and_its_original_deadline() {
        let h = Harness::default();
        let (first, worker) = h.queue("a");
        let task = h.start("a", worker.unwrap(), None, false);
        advance(3000).await;
        let (repeated, worker) = h.queue("a");
        assert!(worker.is_none());
        assert_eq!(repeated.revision, first.revision);
        assert_eq!(repeated.jobs.len(), 1);
        assert_eq!(repeated.jobs[0].job_id, first.jobs[0].job_id);
        assert_eq!(repeated.jobs[0].deadline_unix_ms, first.jobs[0].deadline_unix_ms);
        advance(2000).await;
        assert_eq!(h.deletions.load(Ordering::SeqCst), 1);
        task.await.unwrap();
        assert_eq!(h.deletions.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn stale_undo_and_timer_cannot_touch_a_rescheduled_job() {
        let h = Harness::default();
        let (first, old_worker) = h.queue("a");
        // Retain the old sender so the stale timer must check job ownership,
        // rather than exiting merely because its cancellation channel closed.
        let old_sender = h.state.inner.lock().unwrap().jobs.get_mut("a").unwrap().cancel.take();
        assert_eq!(h.cancel("a", &first.jobs[0].job_id), "cancelled");
        advance(1000).await;
        let (second, worker) = h.queue("a");
        assert_ne!(first.jobs[0].job_id, second.jobs[0].job_id);
        let old_task = h.start("a", old_worker.unwrap(), None, false);
        let task = h.start("a", worker.unwrap(), None, false);
        assert_eq!(h.cancel("a", &first.jobs[0].job_id), "not_found");
        advance(4000).await;
        old_task.await.unwrap();
        assert_eq!(h.deletions.load(Ordering::SeqCst), 0);
        assert_eq!(h.snapshot().jobs[0].job_id, second.jobs[0].job_id);
        advance(1000).await;
        assert_eq!(h.deletions.load(Ordering::SeqCst), 1);
        task.await.unwrap();
        drop(old_sender);
        assert_eq!(h.deletions.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn simultaneous_deletions_wait_for_the_shared_loading_claim() {
        let h = Harness::default();
        let (_, a) = h.queue("a");
        let (_, b) = h.queue("b");
        let (release, wait) = tokio::sync::oneshot::channel();
        let first = h.start("a", a.unwrap(), Some(wait), false);
        advance(5000).await;
        let second = h.start("b", b.unwrap(), None, false);
        tokio::task::yield_now().await;
        assert_eq!(h.deletions.load(Ordering::SeqCst), 1);
        assert_eq!(h.snapshot().jobs.iter().find(|job| job.model_id == "b").unwrap().status, "pending");
        release.send(()).unwrap();
        first.await.unwrap();
        advance(50).await;
        second.await.unwrap();
        assert_eq!(h.deletions.load(Ordering::SeqCst), 2);
        assert!(h.snapshot().jobs.is_empty());
        assert!(!h.statuses().iter().any(|status| status == "failed"));
    }

    #[tokio::test(start_paused = true)]
    async fn undo_remains_available_while_waiting_for_a_busy_loader() {
        let h = Harness::default();
        h.busy.store(true, Ordering::SeqCst);
        let (snapshot, worker) = h.queue("a");
        let task = h.start("a", worker.unwrap(), None, false);
        advance(6000).await;
        assert_eq!(h.snapshot().jobs[0].status, "pending");
        assert_eq!(h.cancel("a", &snapshot.jobs[0].job_id), "cancelled");
        h.busy.store(false, Ordering::SeqCst);
        advance(50).await;
        task.await.unwrap();
        assert_eq!(h.deletions.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn deleting_is_explicitly_too_late_for_undo_or_selection() {
        let h = Harness::default();
        let (snapshot, worker) = h.queue("a");
        let (release, wait) = tokio::sync::oneshot::channel();
        let task = h.start("a", worker.unwrap(), Some(wait), false);
        advance(5000).await;
        assert_eq!(h.cancel("a", &snapshot.jobs[0].job_id), "deleting");
        assert!(h.state.inner.lock().unwrap().cancel_for_use_with("a", &|snapshot| h.publish(snapshot)).is_err());
        assert_eq!(h.snapshot().jobs[0].status, "deleting");
        release.send(()).unwrap();
        task.await.unwrap();
        assert_eq!(h.statuses(), ["pending", "deleting", "completed"]);
    }

    #[tokio::test(start_paused = true)]
    async fn deletion_failure_reports_error_and_releases_claim_for_next_job() {
        let h = Harness::default();
        let (_, worker) = h.queue("a");
        let task = h.start("a", worker.unwrap(), None, true);
        advance(5000).await;
        task.await.unwrap();
        assert!(h.snapshot().jobs.is_empty());
        let change = h.snapshot().change.unwrap();
        assert_eq!(change.status, "failed");
        assert_eq!(change.error.as_deref(), Some("disk failure"));
        assert!(!h.busy.load(Ordering::SeqCst));
        let (_, worker) = h.queue("b");
        let next = h.start("b", worker.unwrap(), None, false);
        advance(5000).await;
        next.await.unwrap();
        assert_eq!(h.deletions.load(Ordering::SeqCst), 2);
        assert_eq!(h.snapshot().change.unwrap().status, "completed");
    }
}

#[derive(Clone, Serialize, Type)]
pub struct ModelDeletionJob {
    pub model_id: String,
    pub job_id: String,
    pub status: String,
    pub deadline_unix_ms: u64,
    pub remaining_ms: u64,
}

#[derive(Clone, Serialize, Type)]
pub struct ModelDeletionChange {
    pub model_id: String,
    pub job_id: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Type)]
pub struct ModelDeletionSnapshot {
    pub revision: u64,
    pub jobs: Vec<ModelDeletionJob>,
    pub change: Option<ModelDeletionChange>,
}

#[derive(Serialize, Type)]
pub struct ModelDeletionCancellation {
    pub outcome: String,
    pub snapshot: ModelDeletionSnapshot,
}

struct ScheduledModelDeletion {
    job: ModelDeletionJob,
    deadline: Instant,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Default)]
struct ModelDeletionInner {
    revision: u64,
    next_job_id: u64,
    jobs: BTreeMap<String, ScheduledModelDeletion>,
    change: Option<ModelDeletionChange>,
}

#[derive(Default)]
pub struct ModelDeletionState {
    inner: Mutex<ModelDeletionInner>,
}

impl ModelDeletionInner {
    fn schedule(
        &mut self,
        model_id: &str,
        now: impl FnOnce() -> Instant,
        unix_ms: impl FnOnce() -> u64,
        validate: impl FnOnce() -> Result<(), String>,
        publish: &impl Fn(ModelDeletionSnapshot),
    ) -> Result<(ModelDeletionSnapshot, Option<(String, Instant, tokio::sync::oneshot::Receiver<()>)>), String> {
        if let Some(scheduled) = self.jobs.get(model_id) {
            return if scheduled.job.status == "pending" {
                Ok((self.snapshot(), None))
            } else {
                Err("Model deletion has already started".to_string())
            };
        }
        validate()?;
        self.next_job_id += 1;
        let job_id = self.next_job_id.to_string();
        let deadline = now() + MODEL_DELETION_DELAY;
        let (cancel, cancellation) = tokio::sync::oneshot::channel();
        self.jobs.insert(model_id.to_string(), ScheduledModelDeletion {
            job: ModelDeletionJob {
                model_id: model_id.to_string(),
                job_id: job_id.clone(),
                status: "pending".to_string(),
                deadline_unix_ms: unix_ms() + MODEL_DELETION_DELAY.as_millis() as u64,
                remaining_ms: MODEL_DELETION_DELAY.as_millis() as u64,
            },
            deadline,
            cancel: Some(cancel),
        });
        let snapshot = self.changed(publish, model_id, &job_id, "pending", None);
        Ok((snapshot, Some((job_id, deadline, cancellation))))
    }

    fn cancel_job(&mut self, model_id: &str, job_id: &str, publish: &impl Fn(ModelDeletionSnapshot)) -> &'static str {
        match self.jobs.get(model_id) {
            Some(scheduled) if scheduled.job.job_id == job_id => {
                if scheduled.job.status == "deleting" {
                    "deleting"
                } else {
                    // This token still owns the pending entry while the mutex is held.
                    let _ = self.cancel_for_use_with(model_id, publish);
                    "cancelled"
                }
            }
            _ => "not_found",
        }
    }

    fn snapshot(&self) -> ModelDeletionSnapshot {
        let now = Instant::now();
        ModelDeletionSnapshot {
            revision: self.revision,
            jobs: self.jobs.values().map(|scheduled| {
                let mut job = scheduled.job.clone();
                job.remaining_ms = if job.status == "pending" {
                    scheduled.deadline.saturating_duration_since(now).as_millis() as u64
                } else {
                    0
                };
                job
            }).collect(),
            change: self.change.clone(),
        }
    }

    fn changed(
        &mut self,
        publish: &impl Fn(ModelDeletionSnapshot),
        model_id: &str,
        job_id: &str,
        status: &str,
        error: Option<String>,
    ) -> ModelDeletionSnapshot {
        self.revision += 1;
        self.change = Some(ModelDeletionChange {
            model_id: model_id.to_string(),
            job_id: job_id.to_string(),
            status: status.to_string(),
            error,
        });
        let snapshot = self.snapshot();
        // Transition and publication share this short lock; no lock crosses an await.
        publish(snapshot.clone());
        snapshot
    }

    fn cancel_for_use(&mut self, app: &AppHandle, model_id: &str) -> Result<(), String> {
        self.cancel_for_use_with(model_id, &|snapshot| {
            let _ = app.emit("model-deletion-state", snapshot);
        })
    }

    fn cancel_for_use_with(&mut self, model_id: &str, publish: &impl Fn(ModelDeletionSnapshot)) -> Result<(), String> {
        let Some(scheduled) = self.jobs.get(model_id) else {
            return Ok(());
        };
        if scheduled.job.status == "deleting" {
            return Err("Model deletion has already started".to_string());
        }
        let job_id = scheduled.job.job_id.clone();
        if let Some(mut scheduled) = self.jobs.remove(model_id) {
            if let Some(cancel) = scheduled.cancel.take() {
                let _ = cancel.send(());
            }
        }
        self.changed(publish, model_id, &job_id, "cancelled", None);
        Ok(())
    }
}

/// Selection and the timer's deleting claim use the same short critical section.
/// The caller retains the returned loading claim until model/settings work finishes.
fn claim_model_selection(app: &AppHandle, model_id: &str) -> Result<LoadingGuard, String> {
    let transcription_manager = app.state::<Arc<TranscriptionManager>>();
    if let Some(state) = app.try_state::<Arc<ModelDeletionState>>() {
        let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
        inner.cancel_for_use(app, model_id)?;
        return transcription_manager.try_start_loading()
            .ok_or_else(|| "Model load already in progress".to_string());
    }
    transcription_manager.try_start_loading()
        .ok_or_else(|| "Model load already in progress".to_string())
}

pub(crate) fn cancel_pending_model_deletion_for_use(
    app: &AppHandle,
    model_id: &str,
) -> Result<(), String> {
    let Some(state) = app.try_state::<Arc<ModelDeletionState>>() else {
        return Ok(());
    };
    let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
    inner.cancel_for_use(app, model_id)
}

pub(crate) fn with_model_deletion_cancelled_for_selection<T>(
    app: &AppHandle,
    model_id: &str,
    select: impl FnOnce() -> T,
) -> Result<T, String> {
    let Some(state) = app.try_state::<Arc<ModelDeletionState>>() else {
        return Ok(select());
    };
    let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
    inner.cancel_for_use(app, model_id)?;
    Ok(select())
}

#[tauri::command]
#[specta::specta]
pub fn get_model_deletion_snapshot(
    deletion_state: State<'_, Arc<ModelDeletionState>>,
) -> ModelDeletionSnapshot {
    let inner = deletion_state.inner.lock().unwrap_or_else(|error| error.into_inner());
    inner.snapshot()
}

#[tauri::command]
#[specta::specta]
pub fn cancel_model_deletion(
    app_handle: AppHandle,
    deletion_state: State<'_, Arc<ModelDeletionState>>,
    model_id: String,
    job_id: String,
) -> ModelDeletionCancellation {
    let mut inner = deletion_state.inner.lock().unwrap_or_else(|error| error.into_inner());
    let outcome = inner.cancel_job(&model_id, &job_id, &|snapshot| {
        let _ = app_handle.emit("model-deletion-state", snapshot);
    });
    ModelDeletionCancellation { outcome: outcome.to_string(), snapshot: inner.snapshot() }
}

#[tauri::command]
#[specta::specta]
pub fn schedule_model_deletion(
    app_handle: AppHandle,
    deletion_state: State<'_, Arc<ModelDeletionState>>,
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<ModelDeletionSnapshot, String> {
    let state = deletion_state.inner().clone();
    let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
    let publisher_app = app_handle.clone();
    let publish = move |snapshot| { let _ = publisher_app.emit("model-deletion-state", snapshot); };
    let (snapshot, worker) = inner.schedule(&model_id, Instant::now, unix_ms_now, || {
        let info = model_manager.get_model_info(&model_id)
            .ok_or_else(|| format!("Model not found: {model_id}"))?;
        if info.is_downloading {
            return Err("Cannot delete a model while its download is in progress".to_string());
        }
        if !info.is_downloaded {
            return Err("Model is not downloaded".to_string());
        }
        Ok(())
    }, &publish)?;
    let Some((job_id, deadline, cancellation)) = worker else { return Ok(snapshot); };
    drop(inner);

    let claim_app = app_handle.clone();
    let delete_id = model_id.clone();
    tauri::async_runtime::spawn(run_model_deletion(state, model_id, job_id, deadline, cancellation, publish,
        move || claim_app.state::<Arc<TranscriptionManager>>().try_start_loading(),
        move |loading_guard| async move {
            tauri::async_runtime::spawn_blocking(move || {
                delete_model_with_loading_claim(&app_handle, &delete_id, loading_guard)
            }).await.map_err(|error| format!("Model deletion task failed: {error}"))
                .and_then(|result| result)
        },
    ));
    Ok(snapshot)
}

async fn run_model_deletion<G, F: std::future::Future<Output = Result<(), String>>>(
    state: Arc<ModelDeletionState>,
    model_id: String,
    job_id: String,
    deadline: Instant,
    mut cancellation: tokio::sync::oneshot::Receiver<()>,
    publish: impl Fn(ModelDeletionSnapshot),
    claim: impl Fn() -> Option<G>,
    delete: impl FnOnce(G) -> F,
) {
    tokio::select! {
        _ = &mut cancellation => return,
        _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {}
    }
    let loading_guard = loop {
        let loading_guard = {
            let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
            let Some(scheduled) = inner.jobs.get(&model_id) else { return; };
            if scheduled.job.job_id != job_id || scheduled.job.status != "pending" {
                return;
            }
            let guard = claim();
            if guard.is_some() {
                let scheduled = inner.jobs.get_mut(&model_id).unwrap();
                scheduled.job.status = "deleting".to_string();
                scheduled.cancel = None;
                inner.changed(&publish, &model_id, &job_id, "deleting", None);
            }
            guard
        };
        if let Some(guard) = loading_guard {
            break guard;
        }
        tokio::select! {
            _ = &mut cancellation => return,
            _ = tokio::time::sleep(Duration::from_millis(50)) => {}
        }
    };
    let result = delete(loading_guard).await;

    let mut inner = state.inner.lock().unwrap_or_else(|error| error.into_inner());
    if inner.jobs.get(&model_id).map(|scheduled| scheduled.job.job_id.as_str())
        != Some(job_id.as_str()) {
        return;
    }
    inner.jobs.remove(&model_id);
    match result {
        Ok(()) => { inner.changed(&publish, &model_id, &job_id, "completed", None); }
        Err(error) => { inner.changed(&publish, &model_id, &job_id, "failed", Some(error)); }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_available_models(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<ModelInfo>, String> {
    Ok(model_manager.get_available_models())
}

#[tauri::command]
#[specta::specta]
pub fn take_missing_model_selection_notice(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Option<(String, Option<String>)> {
    model_manager.take_missing_selection_notice()
}

#[tauri::command]
#[specta::specta]
pub async fn rescan_local_models(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<(), String> {
    let model_manager = model_manager.inner().clone();
    tokio::task::spawn_blocking(move || model_manager.rescan_local_models())
        .await
        .map_err(|e| format!("rescan task panicked: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn download_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    let result = model_manager
        .download_model(&model_id)
        .await
        .map_err(|e| e.to_string());

    if let Err(ref error) = result {
        let _ = app_handle.emit(
            "model-download-failed",
            serde_json::json!({ "model_id": &model_id, "error": error }),
        );
    }

    result
}

/// Ensures a selectable local model is installed before a workflow uses it.
/// A model already being downloaded elsewhere is awaited instead of starting
/// a competing download of the same files.
pub(crate) async fn ensure_local_model_downloaded_for_use(
    app: &AppHandle,
    model_id: &str,
) -> Result<(), String> {
    let model_manager = app.state::<Arc<ModelManager>>();
    let mut waited_for_existing_download = false;

    loop {
        let model_info = model_manager
            .get_model_info(model_id)
            .ok_or_else(|| format!("Model not found: {model_id}"))?;
        if model_info.is_downloaded {
            return Ok(());
        }
        if model_info.is_downloading {
            waited_for_existing_download = true;
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        }
        if waited_for_existing_download {
            return Err(format!("Model download did not complete: {model_id}"));
        }

        let _ = app.emit(
            "model-download-on-demand",
            serde_json::json!({
                "model_id": model_id,
                "model_name": model_info.name,
            }),
        );
        let result = model_manager
            .download_model(model_id)
            .await
            .map_err(|error| error.to_string());
        if let Err(ref error) = result {
            let _ = app.emit(
                "model-download-failed",
                serde_json::json!({ "model_id": model_id, "error": error }),
            );
        }
        result?;
        return model_manager
            .get_model_info(model_id)
            .filter(|model| model.is_downloaded)
            .map(|_| ())
            .ok_or_else(|| format!("Model download did not complete: {model_id}"));
    }
}

#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    app_handle: AppHandle,
    _model_manager: State<'_, Arc<ModelManager>>,
    _transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    cancel_pending_model_deletion_for_use(&app_handle, &model_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        // Immediate callers cancel an older timer before claiming the same loading lane.
        let loading_guard = claim_model_selection(&app_handle, &model_id)?;
        delete_model_with_loading_claim(&app_handle, &model_id, loading_guard)
    }).await.map_err(|error| format!("Model deletion task failed: {error}"))?
}

pub(super) fn delete_model_for_cleanup(app: &AppHandle, model_id: &str) -> Result<(), String> {
    // The same startup claim used by recording closes the check/start race.
    // Active work is rejected; cleanup never cancels a recording or inference.
    let _startup = {
        let state = app.state::<crate::session_manager::ManagedSessionState>();
        let session = state.lock().map_err(|error| error.to_string())?;
        if !matches!(*session, crate::session_manager::SessionState::Idle)
            || crate::session_manager::recording_start_is_blocked()
        {
            return Err("Wait for recording and processing to finish before cleaning up STT models".into());
        }
        crate::session_manager::try_begin_recording_startup()
            .ok_or_else(|| "Recording is starting; try cleanup again when idle".to_string())?
    };
    super::file_transcription::with_file_transcription_idle(|| {
        let manager = app.state::<Arc<ModelManager>>();
        let model = manager.get_model_info(model_id).ok_or_else(|| "Model no longer exists".to_string())?;
        if model.is_downloading {
            return Err("Cancel the active model download and wait for it to stop before cleanup".into());
        }
        cancel_pending_model_deletion_for_use(app, model_id)?;
        let claim = claim_model_selection(app, model_id)?;
        delete_model_with_loading_claim(app, model_id, claim)
    })
}

fn delete_model_with_loading_claim(
    app_handle: &AppHandle,
    model_id: &str,
    _loading_guard: LoadingGuard,
) -> Result<(), String> {
    let model_manager = app_handle.state::<Arc<ModelManager>>();
    let transcription_manager = app_handle.state::<Arc<TranscriptionManager>>();
    let was_selected = get_settings(app_handle).selected_model == model_id;

    // A custom profile/file override can have a loaded model different from selected_model.
    // Always use the existing safe unload before touching its files, retaining the lane
    // through any rollback so no competing load or switch can replace restoration state.
    let was_loaded = transcription_manager.unload_model_for_deletion(model_id)
        .map_err(|e| format!("Failed to unload model: {}", e))?;

    if was_selected {
        let mut updated_settings = get_settings(app_handle);
        if updated_settings.selected_model != model_id {
            return Err("Active model changed while deletion was starting".to_string());
        }
        updated_settings.selected_model = String::new();
        write_settings(app_handle, updated_settings);
    }

    if let Err(delete_error) = model_manager.delete_model(model_id) {
        let current_settings = get_settings(app_handle);
        if !was_selected || current_settings.selected_model.is_empty() {
            if was_loaded {
                // This operation owns the deleting claim; rollback must bypass the
                // public load path's rejection of other callers during deletion.
                let reload = transcription_manager.restore_model_after_failed_deletion(model_id);
                if let Err(reload_error) = reload {
                    tray::refresh_tray_menu(app_handle, None);
                    return Err(format!(
                        "Failed to delete model: {delete_error}. The model also failed to reload: {reload_error}"
                    ));
                }
            }

            if was_selected {
                let mut restored_settings = get_settings(app_handle);
                if restored_settings.selected_model.is_empty() {
                    restored_settings.selected_model = model_id.to_string();
                    write_settings(app_handle, restored_settings);
                }
            }
        }

        tray::refresh_tray_menu(app_handle, None);
        return Err(delete_error.to_string());
    }

    tray::refresh_tray_menu(app_handle, None);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_model(
    app_handle: AppHandle,
    _model_manager: State<'_, Arc<ModelManager>>,
    _transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    // Cancel on receipt, before a busy blocking pool can postpone the selection worker.
    cancel_pending_model_deletion_for_use(&app_handle, &model_id)?;
    tauri::async_runtime::spawn_blocking(move || switch_active_model(&app_handle, &model_id))
        .await
        .map_err(|error| format!("Failed to switch model: {}", error))?
}

pub fn switch_active_model(app: &AppHandle, model_id: &str) -> Result<(), String> {
    let model_manager = app.state::<Arc<ModelManager>>();
    let transcription_manager = app.state::<Arc<TranscriptionManager>>();

    let _loading_guard = claim_model_selection(app, model_id)?;

    let model_info = model_manager
        .get_model_info(model_id)
        .ok_or_else(|| format!("Model not found: {}", model_id))?;

    let settings = get_settings(app);
    let unload_timeout = settings.model_unload_timeout;
    let previous_model_id = settings.selected_model.clone();
    let previous_provider = settings.transcription_provider;
    let previous_language = settings.selected_language.clone();

    let mut updated_settings = settings;
    updated_settings.selected_model = model_id.to_string();
    updated_settings.transcription_provider = TranscriptionProvider::Local;

    // Reset unsupported language selections so backend engines like Canary
    // never receive stale language codes from the previously active model.
    if updated_settings.selected_language != "auto"
        && !model_info.supported_languages.is_empty()
        && !model_info
            .supported_languages
            .contains(&updated_settings.selected_language)
    {
        log::info!(
            "Resetting language from '{}' to 'auto' (not supported by {})",
            updated_settings.selected_language,
            model_id
        );
        updated_settings.selected_language = "auto".to_string();
    }

    if !model_info.is_downloaded {
        write_settings(app, updated_settings);
        let _ = app.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "selection_changed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );
        tray::refresh_tray_menu(app, None);
        return Ok(());
    }

    if unload_timeout == ModelUnloadTimeout::Immediately {
        if transcription_manager.is_model_loaded() {
            transcription_manager
                .unload_model()
                .map_err(|e| format!("Failed to unload previous model: {}", e))?;
        }

        write_settings(app, updated_settings);

        let _ = app.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "selection_changed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );
        tray::refresh_tray_menu(app, None);
        return Ok(());
    }

    write_settings(app, updated_settings);

    if let Err(err) = transcription_manager.load_model(model_id) {
        let mut reverted_settings = get_settings(app);
        reverted_settings.selected_model = previous_model_id;
        reverted_settings.transcription_provider = previous_provider;
        reverted_settings.selected_language = previous_language;
        write_settings(app, reverted_settings);
        tray::refresh_tray_menu(app, None);
        return Err(err.to_string());
    }

    tray::refresh_tray_menu(app, None);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_current_model(app_handle: AppHandle) -> Result<String, String> {
    let settings = get_settings(&app_handle);
    Ok(settings.selected_model)
}

#[tauri::command]
#[specta::specta]
pub async fn get_transcription_model_status(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<Option<String>, String> {
    Ok(transcription_manager.get_current_model())
}

#[tauri::command]
#[specta::specta]
pub async fn has_any_models_available(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<bool, String> {
    let models = model_manager.get_available_models();
    Ok(models.iter().any(|m| m.is_downloaded))
}

#[derive(Serialize, Type)]
pub struct GpuVramStatus {
    pub is_supported: bool,
    pub adapter_name: Option<String>,
    pub used_bytes: u64,
    pub budget_bytes: u64,
    pub system_used_bytes: u64,
    pub system_free_bytes: u64,
    pub total_vram_bytes: u64,
    pub updated_at_unix_ms: u64,
    pub error: Option<String>,
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(target_os = "windows")]
fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|&ch| ch == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end]).trim().to_string()
}

#[cfg(target_os = "windows")]
fn nt_success(status: windows::Win32::Foundation::NTSTATUS) -> bool {
    status.0 >= 0
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct ActiveGpuVramSnapshot {
    adapter_name: String,
    adapter_luid: windows::Win32::Foundation::LUID,
    process_used_bytes: u64,
    process_budget_bytes: u64,
    total_vram_bytes: u64,
}

#[cfg(target_os = "windows")]
fn query_system_gpu_usage_bytes(adapter_luid: windows::Win32::Foundation::LUID) -> Option<u64> {
    use windows::Wdk::Graphics::Direct3D::{
        D3DKMTCloseAdapter, D3DKMTOpenAdapterFromLuid, D3DKMTQueryVideoMemoryInfo,
        D3DKMT_CLOSEADAPTER, D3DKMT_MEMORY_SEGMENT_GROUP_LOCAL, D3DKMT_OPENADAPTERFROMLUID,
        D3DKMT_QUERYVIDEOMEMORYINFO,
    };
    use windows::Win32::Foundation::HANDLE;

    unsafe {
        let mut open = D3DKMT_OPENADAPTERFROMLUID {
            AdapterLuid: adapter_luid,
            ..Default::default()
        };
        let open_status = D3DKMTOpenAdapterFromLuid(&mut open);
        if !nt_success(open_status) || open.hAdapter == 0 {
            return None;
        }

        let mut query = D3DKMT_QUERYVIDEOMEMORYINFO {
            hProcess: HANDLE(std::ptr::null_mut()),
            hAdapter: open.hAdapter,
            MemorySegmentGroup: D3DKMT_MEMORY_SEGMENT_GROUP_LOCAL,
            PhysicalAdapterIndex: 0,
            ..Default::default()
        };
        let query_status = D3DKMTQueryVideoMemoryInfo(&mut query);

        let _ = D3DKMTCloseAdapter(&D3DKMT_CLOSEADAPTER {
            hAdapter: open.hAdapter,
        });

        if !nt_success(query_status) {
            return None;
        }

        Some(query.CurrentUsage)
    }
}

#[cfg(target_os = "windows")]
fn query_active_gpu_vram() -> Result<ActiveGpuVramSnapshot, String> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIAdapter3, IDXGIFactory6,
        DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
        DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
    };

    unsafe {
        let factory: IDXGIFactory6 = CreateDXGIFactory1::<IDXGIFactory6>()
            .map_err(|e| format!("Failed to create DXGI factory: {e}"))?;

        let mut best: Option<ActiveGpuVramSnapshot> = None;
        let mut adapter_index = 0u32;

        loop {
            let adapter: IDXGIAdapter1 = match factory
                .EnumAdapterByGpuPreference(adapter_index, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE)
            {
                Ok(adapter) => adapter,
                Err(_) => break,
            };
            adapter_index += 1;

            let desc = match adapter.GetDesc1() {
                Ok(desc) => desc,
                Err(_) => continue,
            };

            if (desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32)) != 0 {
                continue;
            }

            let adapter3: IDXGIAdapter3 = match adapter.cast::<IDXGIAdapter3>() {
                Ok(adapter3) => adapter3,
                Err(_) => continue,
            };

            let mut memory_info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            if adapter3
                .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut memory_info)
                .is_err()
            {
                continue;
            }

            let budget_bytes = if memory_info.Budget > 0 {
                memory_info.Budget
            } else {
                desc.DedicatedVideoMemory as u64
            };
            let used_bytes = memory_info.CurrentUsage;
            let total_vram_bytes = if desc.DedicatedVideoMemory > 0 {
                desc.DedicatedVideoMemory as u64
            } else {
                budget_bytes
            };
            let adapter_name = wide_to_string(&desc.Description);
            let adapter_name = if adapter_name.is_empty() {
                format!("GPU {}", adapter_index)
            } else {
                adapter_name
            };

            // Prefer the adapter with the largest total VRAM to avoid
            // startup mis-detection where the UI renderer uses a small iGPU budget.
            // If totals are equal, fall back to process usage/budget.
            let should_replace = match best {
                None => true,
                Some(ref best_snapshot) => {
                    total_vram_bytes > best_snapshot.total_vram_bytes
                        || (total_vram_bytes == best_snapshot.total_vram_bytes
                            && (used_bytes > best_snapshot.process_used_bytes
                                || (used_bytes == best_snapshot.process_used_bytes
                                    && budget_bytes > best_snapshot.process_budget_bytes)))
                }
            };

            if should_replace {
                best = Some(ActiveGpuVramSnapshot {
                    adapter_name,
                    adapter_luid: desc.AdapterLuid,
                    process_used_bytes: used_bytes,
                    process_budget_bytes: budget_bytes,
                    total_vram_bytes,
                });
            }
        }

        best.ok_or_else(|| "No active hardware GPU adapter detected".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_active_gpu_vram_status() -> Result<GpuVramStatus, String> {
    let updated_at_unix_ms = unix_ms_now();

    #[cfg(target_os = "windows")]
    {
        match query_active_gpu_vram() {
            Ok(snapshot) => {
                let system_used_bytes = query_system_gpu_usage_bytes(snapshot.adapter_luid)
                    .unwrap_or(snapshot.process_used_bytes);
                let system_free_bytes = snapshot.total_vram_bytes.saturating_sub(system_used_bytes);

                Ok(GpuVramStatus {
                    is_supported: true,
                    adapter_name: Some(snapshot.adapter_name),
                    used_bytes: snapshot.process_used_bytes,
                    budget_bytes: snapshot.process_budget_bytes,
                    system_used_bytes,
                    system_free_bytes,
                    total_vram_bytes: snapshot.total_vram_bytes,
                    updated_at_unix_ms,
                    error: None,
                })
            }
            Err(error) => Ok(GpuVramStatus {
                is_supported: false,
                adapter_name: None,
                used_bytes: 0,
                budget_bytes: 0,
                system_used_bytes: 0,
                system_free_bytes: 0,
                total_vram_bytes: 0,
                updated_at_unix_ms,
                error: Some(error),
            }),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(GpuVramStatus {
            is_supported: false,
            adapter_name: None,
            used_bytes: 0,
            budget_bytes: 0,
            system_used_bytes: 0,
            system_free_bytes: 0,
            total_vram_bytes: 0,
            updated_at_unix_ms,
            error: Some("VRAM meter is only available on Windows".to_string()),
        })
    }
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_download(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .cancel_download(&model_id)
        .map_err(|e| e.to_string())
}
