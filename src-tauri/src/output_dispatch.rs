//! External scripts must not block GTK's event loop. One worker preserves the
//! delivery order that the main-thread queue provides for native paste methods.

use crate::settings::PasteMethod;
use tauri::AppHandle;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

thread_local! {
    static CANCELLATION: std::cell::RefCell<Option<Arc<AtomicBool>>> = const { std::cell::RefCell::new(None) };
}

pub(crate) fn is_cancelled() -> bool {
    CANCELLATION.with(|flag| flag.borrow().as_ref().is_some_and(|flag| flag.load(Ordering::Acquire)))
}

pub(crate) fn dispatch_cancellable(
    app: &AppHandle,
    method: PasteMethod,
    cancelled: Arc<AtomicBool>,
    task: impl FnOnce() + Send + 'static,
) -> Result<(), String> {
    dispatch(app, method, move || {
        if cancelled.load(Ordering::Acquire) { return; }
        CANCELLATION.with(|flag| *flag.borrow_mut() = Some(cancelled));
        struct ClearCancellation;
        impl Drop for ClearCancellation {
            fn drop(&mut self) { CANCELLATION.with(|flag| *flag.borrow_mut() = None); }
        }
        let _clear = ClearCancellation;
        task();
    })
}

#[cfg(any(target_os = "linux", test))]
type Task = Box<dyn FnOnce() + Send>;

#[cfg(any(target_os = "linux", test))]
fn script_queue() -> &'static std::sync::mpsc::Sender<Task> {
    static QUEUE: std::sync::OnceLock<std::sync::mpsc::Sender<Task>> = std::sync::OnceLock::new();
    QUEUE.get_or_init(|| {
        let (sender, receiver) = std::sync::mpsc::channel::<Task>();
        std::thread::Builder::new().name("external-paste".into()).spawn(move || {
            for task in receiver {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(task)).is_err() {
                    log::error!("External paste task panicked");
                }
            }
        }).expect("create external paste worker");
        sender
    })
}

pub(crate) fn dispatch(
    app: &AppHandle,
    method: PasteMethod,
    task: impl FnOnce() + Send + 'static,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if method == PasteMethod::ExternalScript {
        return script_queue().send(Box::new(task))
            .map_err(|_| "External paste worker is unavailable".to_string());
    }
    #[cfg(not(target_os = "linux"))]
    let _ = method;
    app.run_on_main_thread(task).map_err(|error| error.to_string())
}

/// Freeze Linux's generic paste method before choosing its executor. Windows
/// keeps the existing main-thread task and execution-time settings resolution.
pub(crate) fn dispatch_generic(
    app: &AppHandle,
    method: Option<PasteMethod>,
    task: impl FnOnce(Option<PasteMethod>) + Send + 'static,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let method = Some(method.unwrap_or_else(|| crate::settings::get_settings(app).paste_method));
    dispatch(app, method.unwrap_or(PasteMethod::CtrlV), move || task(method))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_output_tasks_run_in_submission_order() {
        let (sender, receiver) = std::sync::mpsc::channel();
        for value in 0..3 {
            let sender = sender.clone();
            script_queue().send(Box::new(move || {
                sender.send(value).unwrap();
            })).unwrap();
        }
        assert_eq!((0..3).map(|_| receiver.recv().unwrap()).collect::<Vec<_>>(), vec![0, 1, 2]);
    }
}
