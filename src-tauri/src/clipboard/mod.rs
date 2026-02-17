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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePayload {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPayload {
    Text(String),
    Image(ImagePayload),
}

pub type ClipCallback = Arc<dyn Fn(ClipboardPayload) + Send + Sync + 'static>;

pub trait ClipboardService: Send + Sync {
    fn set_payload(&self, payload: &ClipboardPayload) -> Result<(), ClipboardError>;
    fn watch_changes(&self, callback: ClipCallback) -> Result<(), ClipboardError>;
    fn active_bundle_id(&self) -> Option<String>;
}

pub fn should_emit_change(
    previous_signature: &mut Option<String>,
    next_signature: &str,
    last_emitted_at: &mut Instant,
    debounce: Duration,
) -> bool {
    if previous_signature
        .as_deref()
        .map(|previous| previous == next_signature)
        .unwrap_or(false)
    {
        return false;
    }
    if last_emitted_at.elapsed() < debounce {
        *previous_signature = Some(next_signature.to_string());
        return false;
    }

    *previous_signature = Some(next_signature.to_string());
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
        let mut previous = None;
        let mut last = Instant::now();
        let debounce = Duration::from_secs(1);

        let emitted = should_emit_change(&mut previous, "a", &mut last, debounce);
        assert!(!emitted);
    }

    #[test]
    fn emits_after_debounce_window() {
        let mut previous = None;
        let mut last = Instant::now() - Duration::from_secs(2);
        let debounce = Duration::from_millis(100);

        let emitted = should_emit_change(&mut previous, "a", &mut last, debounce);
        assert!(emitted);
    }

    #[test]
    fn identical_signature_is_not_emitted() {
        let mut previous = Some("same".to_string());
        let mut last = Instant::now() - Duration::from_secs(2);
        let debounce = Duration::from_millis(10);
        let emitted = should_emit_change(&mut previous, "same", &mut last, debounce);
        assert!(!emitted);
    }
}
