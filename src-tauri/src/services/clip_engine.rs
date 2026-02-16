use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tracing::error;

use crate::clipboard::ClipboardService;
use crate::db::{Clip, Database, LatestClip};
use crate::error::{AppError, AppResult};
use crate::services::prune::run_prune;
use crate::utils::hash::sha256_hex;

pub struct ClipEngine {
    db: Arc<Database>,
    clipboard: Arc<dyn ClipboardService>,
    app: AppHandle,
}

impl ClipEngine {
    pub fn new(db: Arc<Database>, clipboard: Arc<dyn ClipboardService>, app: AppHandle) -> Self {
        Self { db, clipboard, app }
    }

    pub fn start(self: &Arc<Self>) -> AppResult<()> {
        let engine = Arc::clone(self);
        self.clipboard.watch_changes(Arc::new(move |content| {
            if let Err(err) = engine.process_clip(content) {
                error!("clipboard ingestion failed: {err}");
            }
        }))?;
        Ok(())
    }

    pub fn process_clip(&self, content: String) -> AppResult<Option<Clip>> {
        let settings = self.db.get_settings()?;
        if settings.tracking_paused {
            return Ok(None);
        }

        if should_skip_content(&content, settings.max_clip_bytes) {
            return Ok(None);
        }

        let app_bundle_id = self.app.config().identifier.as_str();
        if let Some(bundle_id) = self.clipboard.active_bundle_id() {
            if should_ignore_bundle(
                &bundle_id,
                app_bundle_id,
                &settings.denylist_bundle_ids,
            ) {
                return Ok(None);
            }
        }

        let hash = sha256_hex(&content);
        let latest = self.db.latest_clip()?;
        if is_duplicate(latest.as_ref(), &content, &hash) {
            return Ok(None);
        }

        let content_type = classify_content_type(&content);
        let clip = self.db.insert_clip(&content, content_type)?;
        let _ = run_prune(&self.db, settings.history_limit)?;
        let _ = self.app.emit("clips://created", clip.clone());

        Ok(Some(clip))
    }

    pub fn copy_clip(&self, id: i64) -> AppResult<()> {
        let clip = self.db.get_clip(id)?.ok_or(AppError::NotFound)?;
        self.clipboard.set_content(&clip.content)?;
        Ok(())
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }
}

pub fn should_skip_content(content: &str, max_clip_bytes: i64) -> bool {
    content.trim().is_empty() || content.len() as i64 > max_clip_bytes
}

pub fn should_ignore_bundle(bundle_id: &str, app_bundle_id: &str, denylist: &[String]) -> bool {
    bundle_id == app_bundle_id || denylist.iter().any(|item| item == bundle_id)
}

pub fn is_duplicate(latest: Option<&LatestClip>, content: &str, hash: &str) -> bool {
    latest
        .map(|entry| entry.content == content && entry.hash == hash)
        .unwrap_or(false)
}

pub fn classify_content_type(content: &str) -> &'static str {
    let lower = content.trim().to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return "url";
    }

    let code_signals = [
        "fn ", "const ", "let ", "class ", "import ", "#include", "public ", "private ", "=>",
    ];
    if content.contains('{')
        || content.contains('}')
        || content.contains(';')
        || code_signals.iter().any(|signal| lower.contains(signal))
    {
        return "code";
    }

    "text"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_url() {
        assert_eq!(classify_content_type("https://example.com"), "url");
    }

    #[test]
    fn classifies_code() {
        assert_eq!(classify_content_type("fn main() { println!(\"x\"); }"), "code");
    }

    #[test]
    fn skips_empty_and_oversized() {
        assert!(should_skip_content("   ", 100));
        assert!(should_skip_content(&"a".repeat(20), 10));
        assert!(!should_skip_content("hello", 100));
    }

    #[test]
    fn duplicate_check_matches_latest() {
        let latest = LatestClip {
            content: "hello".to_string(),
            hash: "abc".to_string(),
        };
        assert!(is_duplicate(Some(&latest), "hello", "abc"));
        assert!(!is_duplicate(Some(&latest), "hello!", "abc"));
    }

    #[test]
    fn ignore_bundle_for_self_and_denylist() {
        let denylist = vec!["com.secrets.app".to_string()];
        assert!(should_ignore_bundle(
            "com.klippy.app",
            "com.klippy.app",
            &denylist
        ));
        assert!(should_ignore_bundle(
            "com.secrets.app",
            "com.klippy.app",
            &denylist
        ));
        assert!(!should_ignore_bundle(
            "com.apple.Terminal",
            "com.klippy.app",
            &denylist
        ));
    }
}
