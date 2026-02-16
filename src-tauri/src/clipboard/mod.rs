use std::sync::Arc;
use std::time::{Duration, Instant};

use thiserror::Error;

pub mod macos;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard command failed: {0}")]
    Command(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type ClipCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

pub trait ClipboardService: Send + Sync {
    fn set_content(&self, content: &str) -> Result<(), ClipboardError>;
    fn watch_changes(&self, callback: ClipCallback) -> Result<(), ClipboardError>;
    fn active_bundle_id(&self) -> Option<String>;
}

pub fn should_emit_change(
    previous_content: &mut String,
    next_content: &str,
    last_emitted_at: &mut Instant,
    debounce: Duration,
) -> bool {
    if next_content == previous_content {
        return false;
    }
    if last_emitted_at.elapsed() < debounce {
        *previous_content = next_content.to_string();
        return false;
    }

    *previous_content = next_content.to_string();
    *last_emitted_at = Instant::now();
    true
}

pub fn default_service() -> Arc<dyn ClipboardService> {
    Arc::new(macos::MacOsClipboard::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debounce_blocks_rapid_changes() {
        let mut previous = String::new();
        let mut last = Instant::now();
        let debounce = Duration::from_secs(1);

        let emitted = should_emit_change(&mut previous, "a", &mut last, debounce);
        assert!(!emitted);
    }

    #[test]
    fn emits_after_debounce_window() {
        let mut previous = String::new();
        let mut last = Instant::now() - Duration::from_secs(2);
        let debounce = Duration::from_millis(100);

        let emitted = should_emit_change(&mut previous, "a", &mut last, debounce);
        assert!(emitted);
    }
}
