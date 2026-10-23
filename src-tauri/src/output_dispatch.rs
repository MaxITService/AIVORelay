//! All output (pastes, output keys, clipboard sessions) runs on one worker thread
//! in submission order. A paste sleeps between its steps; off the main thread those
//! sleeps no longer freeze the app's own windows, so a paste aimed at one of them
//! lands. Input injection that needs the main thread hops there through
//! `clipboard::with_enigo`.
//!
//! The main thread must never wait for this worker or hold an output lock: an
//! output task may be waiting for the main thread at that moment.

use std::sync::{Arc, OnceLock, atomic::{AtomicBool, Ordering}, mpsc};
use std::thread::ThreadId;
use std::time::Duration;

thread_local! {
    static CANCELLATION: std::cell::RefCell<Option<Arc<AtomicBool>>> = const { std::cell::RefCell::new(None) };
    static ON_OUTPUT_WORKER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// Records the main thread, so a wait for output started there is reported.
pub(crate) fn set_main_thread() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
}

pub(crate) fn is_cancelled() -> bool {
    CANCELLATION.with(|flag| flag.borrow().as_ref().is_some_and(|flag| flag.load(Ordering::Acquire)))
}

type Task = Box<dyn FnOnce() + Send>;

fn queue() -> &'static mpsc::Sender<Task> {
    static QUEUE: OnceLock<mpsc::Sender<Task>> = OnceLock::new();
    QUEUE.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Task>();
        std::thread::Builder::new().name("output".into()).spawn(move || {
            ON_OUTPUT_WORKER.with(|flag| flag.set(true));
            for task in receiver {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(task)).is_err() {
                    log::error!("Output task panicked");
                }
            }
        }).expect("create output worker");
        sender
    })
}

/// Queues `task` behind all earlier output. Called from an output task, it runs
/// `task` at once instead (like `run_on_main_thread` on the main thread), so nested
/// output cannot deadlock.
pub(crate) fn run(task: impl FnOnce() + Send + 'static) -> Result<(), String> {
    if ON_OUTPUT_WORKER.with(|flag| flag.get()) {
        task();
        return Ok(());
    }
    queue().send(Box::new(task)).map_err(|_| "Output worker is unavailable".to_string())
}

/// Like `run`, but `task` is skipped if `cancelled` is set before it starts; once
/// running, it sees the flag through `is_cancelled`.
pub(crate) fn run_cancellable(
    cancelled: Arc<AtomicBool>,
    task: impl FnOnce() + Send + 'static,
) -> Result<(), String> {
    run(move || {
        if cancelled.load(Ordering::Acquire) { return; }
        let previous = CANCELLATION.with(|flag| flag.replace(Some(cancelled)));
        struct RestoreCancellation(Option<Arc<AtomicBool>>);
        impl Drop for RestoreCancellation {
            fn drop(&mut self) { CANCELLATION.with(|flag| *flag.borrow_mut() = self.0.take()); }
        }
        let _restore = RestoreCancellation(previous);
        task();
    })
}

/// Runs `f` in output order and waits up to `timeout_ms` for its result. After a
/// timeout `f` stays queued and still runs. Never call this on the main thread.
pub(crate) fn run_sync<T: Send + 'static>(
    timeout_ms: u64,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    if MAIN_THREAD.get() == Some(&std::thread::current().id()) {
        log::warn!("The main thread waits for output; an output task that needs the main thread stalls until the timeout");
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    run(move || {
        let _ = sender.send(f());
    })?;
    receiver.recv_timeout(Duration::from_millis(timeout_ms))
        .map_err(|error| format!("Timed out waiting for output task: {}", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT_MS: u64 = 5_000;

    /// Holds the worker until the returned sender sends (or drops).
    fn block_worker() -> mpsc::Sender<()> {
        let (release, released) = mpsc::channel::<()>();
        let (started, has_started) = mpsc::channel();
        run(move || {
            let _ = started.send(());
            let _ = released.recv_timeout(Duration::from_millis(WAIT_MS));
        }).unwrap();
        has_started.recv_timeout(Duration::from_millis(WAIT_MS)).unwrap();
        release
    }

    #[test]
    fn output_tasks_run_in_submission_order() {
        let (sender, receiver) = mpsc::channel();
        for value in 0..3 {
            let sender = sender.clone();
            run(move || sender.send(value).unwrap()).unwrap();
        }
        let order: Vec<_> = (0..3)
            .map(|_| receiver.recv_timeout(Duration::from_millis(WAIT_MS)).unwrap())
            .collect();
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn output_waits_for_the_task_in_progress() {
        let release = block_worker();
        let (sender, receiver) = mpsc::channel();
        run(move || sender.send(()).unwrap()).unwrap();
        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err(),
            "a key press must not land in the middle of a paste");
        release.send(()).unwrap();
        receiver.recv_timeout(Duration::from_millis(WAIT_MS)).unwrap();
    }

    #[test]
    fn output_queued_from_an_output_task_runs_at_once() {
        let order = run_sync(WAIT_MS, || {
            let (sender, receiver) = mpsc::channel();
            run(move || sender.send(1).unwrap()).unwrap();
            let first = receiver.try_recv().expect("nested run must not wait for the queue");
            let second = run_sync(WAIT_MS, || 2).expect("nested wait must not deadlock");
            vec![first, second]
        }).unwrap();
        assert_eq!(order, vec![1, 2]);
    }

    #[test]
    fn timed_out_output_stays_queued_and_still_runs() {
        let release = block_worker();
        let (sender, receiver) = mpsc::channel();
        assert!(run_sync(20, move || sender.send(()).unwrap()).is_err());
        release.send(()).unwrap();
        receiver.recv_timeout(Duration::from_millis(WAIT_MS))
            .expect("a timed-out task must still run in order");
    }

    #[test]
    fn cancelled_output_is_skipped_before_it_starts_and_seen_while_it_runs() {
        let release = block_worker();
        let skipped = Arc::new(AtomicBool::new(false));
        let ran = Arc::new(AtomicBool::new(false));
        let ran_in_task = Arc::clone(&ran);
        run_cancellable(Arc::clone(&skipped), move || ran_in_task.store(true, Ordering::Release)).unwrap();
        skipped.store(true, Ordering::Release);
        release.send(()).unwrap();

        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_in_task = Arc::clone(&cancelled);
        let (seen, was_seen) = mpsc::channel();
        run_cancellable(Arc::clone(&cancelled), move || {
            seen.send(is_cancelled()).unwrap();
            cancelled_in_task.store(true, Ordering::Release);
            seen.send(is_cancelled()).unwrap();
        }).unwrap();
        assert_eq!(was_seen.recv_timeout(Duration::from_millis(WAIT_MS)).unwrap(), false);
        assert_eq!(was_seen.recv_timeout(Duration::from_millis(WAIT_MS)).unwrap(), true);
        assert!(!ran.load(Ordering::Acquire));
        assert_eq!(run_sync(WAIT_MS, is_cancelled).unwrap(), false, "the flag ends with its task");
    }

    #[test]
    fn a_panicking_output_task_does_not_stop_the_worker() {
        run(|| panic!("output task panic (expected by this test)")).unwrap();
        assert_eq!(run_sync(WAIT_MS, || 5).unwrap(), 5);
    }
}
