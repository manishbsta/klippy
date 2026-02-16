use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::{should_emit_change, ClipCallback, ClipboardError, ClipboardService};

const DEFAULT_POLL_MS: u64 = 220;
const DEBOUNCE_MS: u64 = 120;

#[derive(Debug, Clone)]
pub struct MacOsClipboard {
    poll_ms: u64,
}

impl MacOsClipboard {
    pub fn new() -> Self {
        Self {
            poll_ms: DEFAULT_POLL_MS,
        }
    }

    fn get_via_pbpaste() -> Result<String, ClipboardError> {
        let output = Command::new("pbpaste").output()?;
        if !output.status.success() {
            return Err(ClipboardError::Command("pbpaste exited unsuccessfully".to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn run_osascript(script: &str) -> Result<String, ClipboardError> {
        let output = Command::new("osascript").arg("-e").arg(script).output()?;
        if !output.status.success() {
            return Err(ClipboardError::Command("osascript exited unsuccessfully".to_string()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

impl ClipboardService for MacOsClipboard {
    fn set_content(&self, content: &str) -> Result<(), ClipboardError> {
        let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| ClipboardError::Command("failed to open pbcopy stdin".to_string()))?;
        stdin.write_all(content.as_bytes())?;
        drop(stdin);

        let status = child.wait()?;
        if !status.success() {
            return Err(ClipboardError::Command("pbcopy exited unsuccessfully".to_string()));
        }

        Ok(())
    }

    fn watch_changes(&self, callback: ClipCallback) -> Result<(), ClipboardError> {
        let poll = self.poll_ms;
        thread::spawn(move || {
            let mut previous = String::new();
            let mut last_emitted = Instant::now() - Duration::from_millis(DEBOUNCE_MS * 2);
            let debounce = Duration::from_millis(DEBOUNCE_MS);

            loop {
                if let Ok(next) = Self::get_via_pbpaste() {
                    if should_emit_change(&mut previous, &next, &mut last_emitted, debounce) {
                        callback(next);
                    }
                }
                thread::sleep(Duration::from_millis(poll));
            }
        });
        Ok(())
    }

    fn active_bundle_id(&self) -> Option<String> {
        let script = "tell application \"System Events\" to get bundle identifier of first process whose frontmost is true";
        Self::run_osascript(script).ok().filter(|bundle| !bundle.is_empty())
    }
}
