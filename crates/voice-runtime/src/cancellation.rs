use std::sync::{
    LazyLock,
    atomic::{AtomicU64, Ordering},
};

use tokio::sync::watch;

static GENERATION: AtomicU64 = AtomicU64::new(0);
static CANCELLED_GENERATION: AtomicU64 = AtomicU64::new(u64::MAX);
static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static EVENTS: LazyLock<watch::Sender<u64>> = LazyLock::new(|| {
    let (sender, _) = watch::channel(0);
    sender
});

/// Snapshot the currently active voice-operation generation.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

fn signal_change() {
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    EVENTS.send_replace(sequence);
}

fn begin_operation() -> u64 {
    let next = GENERATION.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    signal_change();
    next
}

/// Cancel the current voice capture/STT generation without advancing it.
///
/// Keeping the cancelled generation current is deliberate: final/partial STT
/// work that starts just after the Stop signal still observes that the same
/// voice turn was cancelled. A new microphone operation advances the generation.
pub fn cancel_current() -> u64 {
    let current = generation();
    CANCELLED_GENERATION.store(current, Ordering::Release);
    signal_change();
    current
}

pub fn is_cancelled(start_generation: u64) -> bool {
    generation() != start_generation
        || CANCELLED_GENERATION.load(Ordering::Acquire) == start_generation
}

pub struct CancellationToken {
    generation: u64,
    events: watch::Receiver<u64>,
}

impl CancellationToken {
    /// Start a new microphone-backed voice operation. Starting a newer operation
    /// invalidates older tokens, which preserves the single-capture contract.
    pub fn subscribe() -> Self {
        let generation = begin_operation();
        Self {
            generation,
            events: EVENTS.subscribe(),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_cancelled(&self) -> bool {
        is_cancelled(self.generation)
    }

    /// Resolves once this token's generation is cancelled or superseded.
    pub async fn cancelled(&mut self) {
        if self.is_cancelled() {
            return;
        }

        while self.events.changed().await.is_ok() {
            if self.is_cancelled() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_stays_sticky_until_next_operation() {
        let mut first = CancellationToken::subscribe();
        let first_generation = first.generation();
        assert!(!first.is_cancelled());

        assert_eq!(cancel_current(), first_generation);
        first.cancelled().await;
        assert!(first.is_cancelled());
        assert!(is_cancelled(first_generation));

        let second = CancellationToken::subscribe();
        assert!(!second.is_cancelled());
        assert_ne!(second.generation(), first_generation);
        assert!(is_cancelled(first_generation));
    }

    #[tokio::test]
    async fn newer_operation_invalidates_older_token() {
        let first = CancellationToken::subscribe();
        let second = CancellationToken::subscribe();

        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
    }
}
