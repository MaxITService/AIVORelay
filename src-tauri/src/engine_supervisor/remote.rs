//! Parent-only compatibility layer for the fork's batch and streaming pipelines.
//! Metadata is owned Rust data; no native handles cross this boundary.
use super::{DeviceSelector, EngineSupervisor, LoadSpec, LoadedInfo, StreamHandle, StreamProgress};
#[cfg(test)]
use super::DeviceInfo;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::marker::PhantomData;
use std::sync::{mpsc, Mutex};
use transcribe_cpp::{Backend, Capabilities, ExtSlot, RunOptions, StreamOptions, StreamText, StreamUpdate, Transcript};

static MODEL_OWNERSHIP: ModelOwnership = ModelOwnership { generation: Mutex::new(0) };

// Keep generation changes and command admission in the same critical section.
// Never hold it while waiting for the worker to load or unload a model.
struct ModelOwnership { generation: Mutex<u64> }
impl ModelOwnership {
    fn load<T>(&self, enqueue: impl FnOnce() -> T) -> (u64, T) {
        let mut generation = self.generation.lock().unwrap_or_else(|e| e.into_inner());
        *generation += 1;
        (*generation, enqueue())
    }
    fn unload<T>(&self, expected: u64, enqueue: impl FnOnce() -> T) -> Option<T> {
        let mut generation = self.generation.lock().unwrap_or_else(|e| e.into_inner());
        if *generation != expected { return None; }
        *generation += 1;
        Some(enqueue())
    }
    fn is_current(&self, expected: u64) -> bool {
        *self.generation.lock().unwrap_or_else(|e| e.into_inner()) == expected
    }
}

fn supervisor() -> &'static EngineSupervisor {
    crate::managers::transcription::native_engine_supervisor()
}

#[cfg(test)]
pub(crate) struct RemoteModelOptions {
    pub backend: Backend,
    pub device: Option<DeviceInfo>,
}

#[derive(Clone)]
pub(crate) struct RemoteModel {
    info: LoadedInfo,
    generation: u64,
}

impl RemoteModel {
    #[cfg(test)]
    pub fn load_with(path: &Path, options: &RemoteModelOptions) -> Result<Self> {
        let device = options.device.as_ref().map(|device| {
            DeviceSelector::Key(device.key.clone())
        }).unwrap_or(DeviceSelector::Auto);
        Self::load(path, options.backend, device)
    }

    pub fn load(path: &Path, backend: Backend, device: DeviceSelector) -> Result<Self> {
        let (generation, answer) = MODEL_OWNERSHIP.load(|| {
            supervisor().queue_load(LoadSpec { path: path.to_path_buf(), backend, device })
        });
        let info = answer.recv().map_err(|_| anyhow!("Native engine supervisor stopped"))??;
        Ok(Self { info, generation })
    }

    pub fn arch(&self) -> String { self.info.arch.clone() }
    pub fn variant(&self) -> String { self.info.variant.clone() }
    pub fn backend(&self) -> String { self.info.backend.clone() }
    pub fn capabilities(&self) -> Capabilities { self.info.capabilities.clone() }
    pub fn accepts_ext(&self, slot: ExtSlot, kind: u32) -> bool {
        matches!(slot, ExtSlot::Stream) && self.info.accepted_stream_extensions.contains(&kind)
    }
    pub fn bound_device(&self) -> &str { &self.info.device }
    pub fn session(&self) -> Result<RemoteSession> { Ok(RemoteSession { model: self.clone() }) }
}

pub(crate) struct RemoteSession { model: RemoteModel }
impl RemoteSession {
    pub fn model(&self) -> RemoteModel { self.model.clone() }
    pub fn run(&mut self, pcm: &[f32], options: &RunOptions) -> Result<Transcript> {
        self.ensure_current()?;
        Ok(supervisor().transcribe(pcm.to_vec(), options.clone())?)
    }
    fn ensure_current(&self) -> Result<()> {
        if !MODEL_OWNERSHIP.is_current(self.model.generation) {
            return Err(anyhow!("Native model session was replaced"));
        }
        Ok(())
    }
    pub fn stream(&mut self, run: &RunOptions, options: &StreamOptions) -> Result<RemoteStream<'_>> {
        self.ensure_current()?;
        let (tx, progress) = mpsc::channel();
        let handle = supervisor().start_stream(run.clone(), options.clone(), move |update| { let _ = tx.send(update); })?;
        Ok(RemoteStream { handle: Some(handle), progress, text: StreamText::default(), language: None, session: PhantomData })
    }
}
impl Drop for RemoteSession {
    fn drop(&mut self) {
        // A late-returning fork engine must never unload a newer model.
        if let Some(unloading) = MODEL_OWNERSHIP.unload(self.model.generation, || supervisor().unload()) {
            unloading.wait();
        }
    }
}

pub(crate) struct RemoteStream<'a> {
    session: PhantomData<&'a mut RemoteSession>,
    handle: Option<StreamHandle>,
    progress: mpsc::Receiver<StreamProgress>,
    text: StreamText,
    language: Option<String>,
}
pub(crate) struct RemoteSnapshot { pub language: Option<String> }
impl RemoteStream<'_> {
    pub fn feed(&mut self, pcm: &[f32]) -> Result<StreamUpdate> {
        self.handle.as_ref().ok_or_else(|| anyhow!("Native stream is closed"))?.feed_and_wait(pcm.to_vec())?;
        let progress = self.progress.try_recv().map_err(|_| anyhow!("Native stream feed failed or was cancelled"))?;
        if let Some(text) = progress.text { self.text = text; }
        Ok(progress.update)
    }
    pub fn text(&self) -> StreamText { self.text.clone() }
    pub fn finalize(&mut self) -> Result<StreamUpdate> {
        let handle = self.handle.take().ok_or_else(|| anyhow!("Native stream is closed"))?;
        let finalized = handle.finalize(true)?.ok_or_else(|| anyhow!("Native stream was lost; retry with batch transcription"))?;
        self.text = finalized.text;
        self.language = finalized.language;
        Ok(finalized.update)
    }
    pub fn snapshot(&self) -> RemoteSnapshot { RemoteSnapshot { language: self.language.clone() } }
    pub fn reset(&mut self) { drop(self.handle.take()); }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn stale_session_cannot_enqueue_unload_after_a_new_load() {
        let ownership = ModelOwnership { generation: Mutex::new(0) };
        let mut queue = Vec::new();
        let (old, ()) = ownership.load(|| queue.push("old load"));
        let (new, ()) = ownership.load(|| queue.push("new load"));
        assert!(ownership.unload(old, || queue.push("stale unload")).is_none());
        assert!(!ownership.is_current(old));
        assert!(ownership.is_current(new));
        assert_eq!(queue, ["old load", "new load"]);
    }

    #[test]
    fn unloading_ownership_is_atomic_with_command_admission() {
        let ownership = Arc::new(ModelOwnership { generation: Mutex::new(0) });
        let queue = Arc::new(Mutex::new(Vec::new()));
        let (old, ()) = ownership.load(|| queue.lock().unwrap().push("old load"));
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let unloading_ownership = ownership.clone();
        let unloading_queue = queue.clone();
        let unloading = std::thread::spawn(move || {
            unloading_ownership.unload(old, || {
                ready_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                unloading_queue.lock().unwrap().push("old unload");
            })
        });
        ready_rx.recv().unwrap();
        // The old drop has claimed its generation but has not enqueued yet.
        // A concurrent load must not advance ownership in this window.
        assert!(ownership.generation.try_lock().is_err());
        let loading_ownership = ownership.clone();
        let loading_queue = queue.clone();
        let loading = std::thread::spawn(move || {
            loading_ownership.load(|| loading_queue.lock().unwrap().push("new load"))
        });
        release_tx.send(()).unwrap();
        assert_eq!(unloading.join().unwrap(), Some(()));
        let (new, ()) = loading.join().unwrap();
        assert!(ownership.is_current(new));
        assert_eq!(*queue.lock().unwrap(), ["old load", "old unload", "new load"]);
    }
}
