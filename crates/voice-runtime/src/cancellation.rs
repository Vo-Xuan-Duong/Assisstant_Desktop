use std::sync::{
    LazyLock,
    atomic::{AtomicU64, Ordering},
};

use tokio::sync::watch;

static GENERATION: AtomicU64 = AtomicU64::new(0);
static EVENTS: LazyLock<watch::Sender<u64>> = LazyLock::new(|| {
    let (sender, _) = watch::channel(0);
    sender
});

/// Snapshot the current voice-operation cancellation generation.
/// Work started under this value is stale as soon as `cancel_current()` bumps it.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Cancel voice capture/STT work that started before this call.
///
/// This is intentionally separate from Assistant Core cancellation: native audio
/// and offline STT need a lightweight out-of-band signal that does not wait on
/// the desktop turn/session mutexes.
pub fn cancel_current() -> u64 {
    let next = GENERATION.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    EVENTS.send_replace(next);
    next
}

pub fn is_cancelled(start_generation: u64) -> bool {
    generation() != start_generation
}

pub struct CancellationToken {
    generation: u64,
    events: watch::Receiver<u64>,
}

impl CancellationToken {
    pub fn subscribe() -> Self {
        let generation = generation();
        Self {
            generation,
            events: EVENTS.subscribe(),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_cancelled(&self) -> bool {
        *self.events.borrow() != self.generation || is_cancelled(self.generation)
    }

    /// Resolves once this token's generation is no longer current.
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
    async fn cancellation_invalidates_existing_token_only() {
        let mut first = CancellationToken::subscribe();
        assert!(!first.is_cancelled());

        cancel_current();
        first.cancelled().await;
        assert!(first.is_cancelled());

        let second = CancellationToken::subscribe();
        assert!(!second.is_cancelled());
    }
}
