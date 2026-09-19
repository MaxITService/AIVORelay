//! Keeps the UI alive when the WebView2 browser process dies.
//!
//! Every webview window shares one WebView2 browser process. When that process
//! is killed, each window keeps its native frame but renders black, the
//! overlays never appear, and every later WebView2 call fails with
//! `ERROR_INVALID_STATE` (`0x8007139F`). Hiding and re-showing a window does
//! not help; only a new WebView2 controller starts a new browser process.
//!
//! The known trigger is third-party software injecting hook DLLs into
//! `msedgewebview2.exe` (RivaTuner's `RTSSHooks64.dll` crashed the browser
//! process a few seconds after startup, see
//! `.AGENTS/.untracked/webview2-crash-20260919`). This module reports the
//! failure with the details WebView2 provides and rebuilds the windows, with a
//! budget so a crash loop cannot turn into an endless rebuild loop.

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, WebviewWindow};
#[cfg(target_os = "windows")]
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2ProcessFailedEventArgs, COREWEBVIEW2_PROCESS_FAILED_KIND,
    COREWEBVIEW2_PROCESS_FAILED_REASON,
};

/// Automatic repairs allowed inside [`AUTO_REPAIR_WINDOW`]. Every recorded
/// browser crash happened 4-5 s after the browser process started, so a single
/// rebuild normally fixes the session; three in ten minutes means the crash is
/// systematic and repeating it would only flicker windows forever.
const MAX_AUTO_REPAIRS: usize = 3;
const AUTO_REPAIR_WINDOW: Duration = Duration::from_secs(10 * 60);

/// Destroyed windows leave Tauri's registry only after their `Destroyed`
/// events are processed, so the rebuild polls before reusing the labels.
const DESTROY_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DESTROY_POLL_LIMIT: u32 = 60;

/// Crash records older than this are unrelated to the failure being handled.
const CRASH_RECORD_MAX_AGE: Duration = Duration::from_secs(120);

const MAIN_WINDOW_LABEL: &str = "main";
const VOICE_BUTTON_LABEL: &str = "voice_activation_button";

/// Incremented whenever a rebuild is claimed; failure reports from webviews
/// of an older generation describe the crash already being handled.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static RECOVERING: AtomicBool = AtomicBool::new(false);
/// A fresh browser crash reported while a rebuild was still in flight.
static REBUILD_AGAIN: AtomicBool = AtomicBool::new(false);
static AUTO_REPAIRS: Mutex<Vec<Instant>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, PartialEq, Eq)]
enum RebuildTrigger {
    /// WebView2 reported that its browser process is gone.
    BrowserProcessExited,
    /// The user asked for the main window while none exists.
    UserRequest,
    /// The automatic budget is exhausted: tear the dead windows down and stop.
    GiveUp,
}

struct RebuildPlan {
    trigger: RebuildTrigger,
    generation: u64,
    main_was_visible: bool,
    main_was_minimized: bool,
    respawn_voice_button: bool,
    destroyed_labels: Vec<String>,
}

/// True while windows are being torn down and rebuilt. During that moment the
/// app may briefly have no windows, which must not be mistaken for "all
/// windows closed".
pub fn is_recovering() -> bool {
    RECOVERING.load(Ordering::SeqCst)
}

/// Rebuilds the webviews on the user's behalf when the main window is missing,
/// e.g. after automatic recovery gave up. User requests are not budgeted:
/// each one is an explicit click.
pub fn rebuild_for_user(app: &AppHandle) {
    if crate::webview_mode::webviews_disabled() {
        return;
    }
    if RECOVERING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::info!("Main window requested while the webviews are already being rebuilt");
        return;
    }
    GENERATION.fetch_add(1, Ordering::SeqCst);
    log::warn!("Main window is missing; rebuilding the webviews on request");
    post_to_main_thread(app.clone(), Duration::ZERO, |app| {
        begin_rebuild(app, RebuildTrigger::UserRequest)
    });
}

/// Registers a WebView2 `ProcessFailed` listener on a freshly built window.
#[cfg(target_os = "windows")]
pub fn watch(window: &WebviewWindow) {
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    let generation = GENERATION.load(Ordering::SeqCst);

    let result = window.with_webview(move |webview| {
        use webview2_com::ProcessFailedEventHandler;

        // SAFETY: plain COM calls on the controller Tauri hands out for this
        // window; the handler owns its captures and outlives the registration.
        let core = match unsafe { webview.controller().CoreWebView2() } {
            Ok(core) => core,
            Err(err) => {
                log::warn!("Cannot watch WebView2 process failures for '{label}': {err}");
                return;
            }
        };

        let handler_label = label.clone();
        let handler = ProcessFailedEventHandler::create(Box::new(move |sender, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let failure = ProcessFailure::read(&args);
            on_process_failed(&app, &handler_label, generation, sender, failure);
            Ok(())
        }));

        let mut token = 0i64;
        if let Err(err) = unsafe { core.add_ProcessFailed(&handler, &mut token) } {
            log::warn!("Cannot watch WebView2 process failures for '{label}': {err}");
        }
    });

    if let Err(err) = result {
        log::warn!(
            "Failed to access WebView2 instance for '{}': {}",
            window.label(),
            err
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn watch(_window: &WebviewWindow) {}

#[cfg(target_os = "windows")]
struct ProcessFailure {
    kind: COREWEBVIEW2_PROCESS_FAILED_KIND,
    reason: Option<COREWEBVIEW2_PROCESS_FAILED_REASON>,
    exit_code: Option<i32>,
    description: Option<String>,
}

#[cfg(target_os = "windows")]
impl ProcessFailure {
    fn read(args: &ICoreWebView2ProcessFailedEventArgs) -> Self {
        use webview2_com::take_pwstr;
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2ProcessFailedEventArgs2;
        use windows::core::{Interface, PWSTR};

        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
        // SAFETY: getters on the event args WebView2 passed to the handler.
        let _ = unsafe { args.ProcessFailedKind(&mut kind) };
        let mut failure = Self {
            kind,
            reason: None,
            exit_code: None,
            description: None,
        };

        // Reason, exit code and process description need a newer runtime.
        let Ok(details) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() else {
            return failure;
        };
        unsafe {
            let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON::default();
            if details.Reason(&mut reason).is_ok() {
                failure.reason = Some(reason);
            }
            let mut exit_code = 0i32;
            if details.ExitCode(&mut exit_code).is_ok() {
                failure.exit_code = Some(exit_code);
            }
            let mut description = PWSTR::null();
            if details.ProcessDescription(&mut description).is_ok() && !description.is_null() {
                let description = take_pwstr(description);
                if !description.is_empty() {
                    failure.description = Some(description);
                }
            }
        }

        failure
    }

    fn kind_name(&self) -> String {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;

        let name = match self.kind {
            COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED => "browser process exited",
            COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED => "render process exited",
            COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE => {
                "render process unresponsive"
            }
            COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED => {
                "frame render process exited"
            }
            COREWEBVIEW2_PROCESS_FAILED_KIND_UTILITY_PROCESS_EXITED => "utility process exited",
            COREWEBVIEW2_PROCESS_FAILED_KIND_SANDBOX_HELPER_PROCESS_EXITED => {
                "sandbox helper process exited"
            }
            COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED => "GPU process exited",
            COREWEBVIEW2_PROCESS_FAILED_KIND_PPAPI_PLUGIN_PROCESS_EXITED => {
                "PPAPI plugin process exited"
            }
            COREWEBVIEW2_PROCESS_FAILED_KIND_PPAPI_BROKER_PROCESS_EXITED => {
                "PPAPI broker process exited"
            }
            COREWEBVIEW2_PROCESS_FAILED_KIND_UNKNOWN_PROCESS_EXITED => "unknown process exited",
            other => return format!("process failure kind {}", other.0),
        };
        name.to_string()
    }

    fn reason_name(&self) -> Option<&'static str> {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;

        Some(match self.reason? {
            COREWEBVIEW2_PROCESS_FAILED_REASON_UNEXPECTED => "unexpected",
            COREWEBVIEW2_PROCESS_FAILED_REASON_UNRESPONSIVE => "unresponsive",
            COREWEBVIEW2_PROCESS_FAILED_REASON_TERMINATED => "terminated",
            COREWEBVIEW2_PROCESS_FAILED_REASON_CRASHED => "crashed",
            COREWEBVIEW2_PROCESS_FAILED_REASON_LAUNCH_FAILED => "launch failed",
            COREWEBVIEW2_PROCESS_FAILED_REASON_OUT_OF_MEMORY => "out of memory",
            COREWEBVIEW2_PROCESS_FAILED_REASON_PROFILE_DELETED => "profile deleted",
            _ => "other",
        })
    }

    fn summary(&self) -> String {
        let mut summary = self.kind_name();
        if let Some(reason) = self.reason_name() {
            summary.push_str(&format!(", reason: {reason}"));
        }
        if let Some(exit_code) = self.exit_code {
            summary.push_str(&format!(
                ", exit code: {exit_code} (0x{:08X})",
                exit_code as u32
            ));
        }
        if let Some(description) = &self.description {
            summary.push_str(&format!(", process: {description}"));
        }
        summary
    }
}

/// Runs inside WebView2's event dispatch on the main thread. It must not tear
/// anything down itself: the rebuild is posted for the next event-loop turn.
#[cfg(target_os = "windows")]
fn on_process_failed(
    app: &AppHandle,
    label: &str,
    generation: u64,
    sender: Option<ICoreWebView2>,
    failure: ProcessFailure,
) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE,
    };

    let stale = generation != GENERATION.load(Ordering::SeqCst);
    let summary = failure.summary();

    match failure.kind {
        COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED => {
            if stale {
                log::debug!(
                    "Ignoring a WebView2 browser failure from the replaced window '{label}'"
                );
                return;
            }
            log::error!(
                "WebView2 browser process died (reported by window '{label}'): {summary}. \
                 Every window and overlay is blank until the webviews are rebuilt."
            );
            schedule_rebuild(app);
        }
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED => {
            log::error!("WebView2 page process failed in window '{label}': {summary}");
            if stale {
                return;
            }
            if !take_auto_repair() {
                log::error!(
                    "Not reloading '{label}': {MAX_AUTO_REPAIRS} automatic WebView2 repairs \
                     were already used within {} minutes",
                    AUTO_REPAIR_WINDOW.as_secs() / 60
                );
                return;
            }
            // WebView2 guidance: a dead render process is recovered by Reload().
            log::info!("Reloading window '{label}' after its page process failed");
            if let Some(core) = sender {
                if let Err(err) = unsafe { core.Reload() } {
                    log::error!("Failed to reload window '{label}': {err}");
                }
            }
        }
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE => {
            log::warn!("WebView2 page in window '{label}' is not responding: {summary}");
        }
        _ => {
            // WebView2 restarts its GPU, utility and helper processes itself.
            log::warn!("WebView2 process failure in window '{label}': {summary}");
        }
    }
}

fn schedule_rebuild(app: &AppHandle) {
    if RECOVERING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        // Only a window of the current generation gets here, so this is a
        // new crash that hit while the previous rebuild was still running
        // (a crash during window creation). Repeat once the rebuild ends.
        REBUILD_AGAIN.store(true, Ordering::SeqCst);
        return;
    }
    // Every window reports the same browser crash; the first report wins and
    // makes the others stale.
    GENERATION.fetch_add(1, Ordering::SeqCst);

    let trigger = if take_auto_repair() {
        RebuildTrigger::BrowserProcessExited
    } else {
        log::error!(
            "WebView2 browser process died {MAX_AUTO_REPAIRS} times within {} minutes; \
             giving up on automatic recovery and closing the dead windows. Fix or exclude \
             the software injecting into msedgewebview2.exe, then reopen AivoRelay from \
             the tray icon or restart it.",
            AUTO_REPAIR_WINDOW.as_secs() / 60
        );
        RebuildTrigger::GiveUp
    };

    post_to_main_thread(app.clone(), Duration::ZERO, move |app| {
        begin_rebuild(app, trigger)
    });
}

/// Consumes one automatic repair from the sliding budget.
fn take_auto_repair() -> bool {
    let now = Instant::now();
    let mut attempts = AUTO_REPAIRS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    attempts.retain(|attempt| now.duration_since(*attempt) < AUTO_REPAIR_WINDOW);
    if attempts.len() >= MAX_AUTO_REPAIRS {
        return false;
    }
    attempts.push(now);
    true
}

/// `run_on_main_thread` executes its task inline when the caller already is
/// the main thread. Rebuild steps must instead run on a later event-loop turn:
/// the crash is reported from inside a WebView2 callback, and destroying
/// windows there would drop the controller that is dispatching the event.
/// Posting from another thread always goes through the event-loop queue.
fn post_to_main_thread(
    app: AppHandle,
    delay: Duration,
    task: impl FnOnce(AppHandle) + Send + 'static,
) {
    std::thread::spawn(move || {
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        let handle = app.clone();
        if let Err(err) = app.run_on_main_thread(move || task(handle)) {
            log::error!("Failed to post a webview rebuild step to the main thread: {err}");
            RECOVERING.store(false, Ordering::SeqCst);
        }
    });
}

/// Runs on the main thread: records what exists, destroys every webview
/// window, then waits for the labels to free up before rebuilding.
fn begin_rebuild(app: AppHandle, trigger: RebuildTrigger) {
    let generation = GENERATION.load(Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    if trigger != RebuildTrigger::UserRequest {
        log_recent_crash_record(&app);
    }

    let windows = app.webview_windows();
    let main_window = windows.get(MAIN_WINDOW_LABEL);
    let plan = RebuildPlan {
        trigger,
        generation,
        main_was_visible: main_window
            .map(|window| window.is_visible().unwrap_or(false))
            .unwrap_or(false),
        main_was_minimized: main_window
            .map(|window| window.is_minimized().unwrap_or(false))
            .unwrap_or(false),
        respawn_voice_button: windows.contains_key(VOICE_BUTTON_LABEL),
        destroyed_labels: windows.keys().cloned().collect(),
    };

    log::info!(
        "Tearing down {} webview window(s) for rebuild generation {generation}: {:?}",
        windows.len(),
        plan.destroyed_labels
    );
    for (label, window) in windows {
        if let Err(err) = window.destroy() {
            log::warn!("Failed to destroy window '{label}': {err}");
        }
    }

    continue_rebuild(app, plan, 0);
}

fn continue_rebuild(app: AppHandle, plan: RebuildPlan, poll: u32) {
    let remaining: Vec<&String> = plan
        .destroyed_labels
        .iter()
        .filter(|label| app.get_webview_window(label.as_str()).is_some())
        .collect();

    if !remaining.is_empty() {
        if poll >= DESTROY_POLL_LIMIT {
            log::error!(
                "Windows {remaining:?} are still registered {} ms after being destroyed; \
                 aborting the webview rebuild",
                u64::from(poll) * DESTROY_POLL_INTERVAL.as_millis() as u64
            );
            RECOVERING.store(false, Ordering::SeqCst);
            REBUILD_AGAIN.store(false, Ordering::SeqCst);
            return;
        }
        post_to_main_thread(app, DESTROY_POLL_INTERVAL, move |app| {
            continue_rebuild(app, plan, poll + 1)
        });
        return;
    }

    finish_rebuild(&app, plan);
    RECOVERING.store(false, Ordering::SeqCst);
    if REBUILD_AGAIN.swap(false, Ordering::SeqCst) {
        log::warn!("The rebuilt WebView2 browser process died as well; rebuilding again");
        schedule_rebuild(&app);
    }
}

fn finish_rebuild(app: &AppHandle, plan: RebuildPlan) {
    if plan.trigger == RebuildTrigger::GiveUp {
        log::warn!(
            "Dead webview windows closed; AivoRelay keeps running from the tray without a UI"
        );
        return;
    }

    let settings = crate::settings::get_settings(app);
    let user_requested = plan.trigger == RebuildTrigger::UserRequest;
    let main_window = match crate::create_main_window(app, &settings, user_requested) {
        Ok(window) => window,
        Err(err) => {
            log::error!("Failed to rebuild the main window: {err}");
            return;
        }
    };
    crate::utils::create_recording_overlay(app);
    if plan.respawn_voice_button {
        if let Err(err) = crate::overlay::show_voice_activation_button_window(app) {
            log::warn!("Failed to respawn the voice activation button: {err}");
        }
    }
    if user_requested {
        crate::show_main_window(app);
    } else if plan.main_was_minimized {
        // Minimizing the still-hidden window puts it straight on the taskbar:
        // no restored-state flash and no activation.
        if let Err(err) = main_window.minimize() {
            log::warn!("Failed to re-minimize the rebuilt main window: {err}");
        }
    } else if plan.main_was_visible {
        // Built with focus disabled, so this shows without activating.
        if let Err(err) = main_window.show() {
            log::warn!("Failed to show the rebuilt main window: {err}");
        }
    }

    log::info!(
        "Webview windows rebuilt (generation {}); the UI is available again",
        plan.generation
    );
}

/// Names the module WebView2's crash reporter blamed for the latest browser
/// crash, so a support log answers "which software killed the UI" directly.
/// Best effort: the Watson metadata format is Microsoft's, not ours.
#[cfg(target_os = "windows")]
fn log_recent_crash_record(app: &AppHandle) {
    let Ok(runtime) = crate::webview_runtime::config(app) else {
        return;
    };
    // WebView2 keeps its profile in an `EBWebView` folder inside the user
    // data directory it was given.
    let crashpad_dir = runtime.data_directory.join("EBWebView").join("Crashpad");
    let reports_dir = crashpad_dir.join("reports");
    let metadata_path = crashpad_dir.join("watson_metadata");

    let fresh = std::fs::metadata(&metadata_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.elapsed().ok())
        .map(|age| age < CRASH_RECORD_MAX_AGE)
        .unwrap_or(false);
    if !fresh {
        log::info!(
            "No fresh WebView2 crash record; Crashpad reports live in {}",
            reports_dir.display()
        );
        return;
    }

    let Ok(bytes) = std::fs::read(&metadata_path) else {
        return;
    };
    let text = String::from_utf8_lossy(&bytes);
    // Records are appended in order; the last one belongs to this crash. Each
    // is a `key=value;` list terminated by binary data.
    let Some(record) = text
        .rfind("ApplicationName=")
        .map(|start| &text[start..])
        .and_then(|record| {
            record
                .split(|c: char| c.is_control() || !c.is_ascii())
                .next()
        })
    else {
        return;
    };

    let field = |name: &str| {
        record
            .split(';')
            .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
            .filter(|value| !value.is_empty())
    };
    let Some(module) = field("ModuleName") else {
        return;
    };

    log::error!(
        "Latest WebView2 crash record: faulting module {module}{}{}{}. A module that is \
         not part of Windows or WebView2 was injected by other software (overlay, FPS, RGB \
         or capture tools); exclude msedgewebview2.exe there. Reports: {}",
        field("ProcessType")
            .map(|value| format!(", process type {value}"))
            .unwrap_or_default(),
        field("SubCode")
            .map(|value| format!(", exception {value}"))
            .unwrap_or_default(),
        field("ApplicationVersion")
            .map(|value| format!(", runtime {value}"))
            .unwrap_or_default(),
        reports_dir.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_repair_budget_is_a_sliding_window() {
        let Some(expired) = Instant::now().checked_sub(AUTO_REPAIR_WINDOW + Duration::from_secs(1))
        else {
            return;
        };
        {
            let mut attempts = AUTO_REPAIRS.lock().unwrap();
            attempts.clear();
            attempts.extend(std::iter::repeat(expired).take(MAX_AUTO_REPAIRS));
        }
        // Expired attempts do not count against the budget.
        for _ in 0..MAX_AUTO_REPAIRS {
            assert!(take_auto_repair());
        }
        assert!(!take_auto_repair());
        AUTO_REPAIRS.lock().unwrap().clear();
    }
}
