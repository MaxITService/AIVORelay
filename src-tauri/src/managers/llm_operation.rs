//! LLM Operation Tracker
//!
//! Provides cancellation tracking for LLM requests (AI Replace, etc.)
//! Similar pattern to RemoteSttManager's operation tracking.

use std::sync::atomic::{AtomicU64, Ordering};

/// Tracks LLM operations and allows cancellation.
/// When cancel() is called, all operations started before that point are marked as cancelled.
pub struct LlmOperationTracker {
    /// Monotonically increasing operation ID
    current_operation_id: AtomicU64,
    /// Operations with ID less than this value are considered cancelled
    cancelled_before_id: AtomicU64,
}

impl LlmOperationTracker {
    pub fn new() -> Self {
        Self {
            current_operation_id: AtomicU64::new(0),
            cancelled_before_id: AtomicU64::new(0),
        }
    }

    /// Returns a new operation ID for tracking cancellation.
    pub fn start_operation(&self) -> u64 {
        self.current_operation_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Marks all operations started before now as cancelled.
    pub fn cancel(&self) {
        let current = self.current_operation_id.load(Ordering::SeqCst);
        self.cancelled_before_id
            .store(current + 1, Ordering::SeqCst);
        log::info!(
            "LlmOperationTracker: cancelled all operations up to id {}",
            current + 1
        );
    }

    /// Returns true if the given operation ID has been cancelled.
    pub fn is_cancelled(&self, operation_id: u64) -> bool {
        operation_id < self.cancelled_before_id.load(Ordering::SeqCst)
    }
}

impl Default for LlmOperationTracker {
    fn default() -> Self {
        Self::new()
    }
}
