use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use tracing::{error, warn};

use crate::clipboard::{ClipboardPayload, ClipboardService, ImagePayload};
use crate::db::{Clip, Database, ImageClipInsert, LatestClip};
use crate::error::{AppError, AppResult};
use crate::services::media_store::{MediaStore, StoredImage};
use crate::services::prune::run_prune;
use crate::utils::hash::sha256_hex;

const INTERNAL_COPY_SUPPRESS_WINDOW: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone)]
enum PendingInternalPayload {
    Text(String),
    ImageHash(String),
}

#[derive(Debug, Clone)]
struct PendingInternalCopy {
    payload: PendingInternalPayload,
    created_at: Instant,
}

pub struct ClipEngine {
    db: Arc<Database>,
    clipboard: Arc<dyn ClipboardService>,
    media_store: Arc<MediaStore>,
    app: AppHandle,
    pending_internal_copy: Mutex<Option<PendingInternalCopy>>,
}

impl ClipEngine {
    pub fn new(
        db: Arc<Database>,
        clipboard: Arc<dyn ClipboardService>,
        media_store: Arc<MediaStore>,
        app: AppHandle,
    ) -> Self {
        Self {
            db,
            clipboard,
            media_store,
            app,
            pending_internal_copy: Mutex::new(None),
        }
    }

    pub fn start(self: &Arc<Self>) -> AppResult<()> {
        let engine = Arc::clone(self);
        self.clipboard.watch_changes(Arc::new(move |payload| {
            if let Err(err) = engine.process_payload(payload) {
                error!("clipboard ingestion failed: {err}");
            }
        }))?;
        Ok(())
    }

    pub fn process_payload(&self, payload: ClipboardPayload) -> AppResult<Option<Clip>> {
        let settings = self.db.get_settings()?;
        if should_skip_payload(&payload, settings.max_clip_bytes) {
            return Ok(None);
        }

        if self.should_skip_pending_internal_copy(&payload)? {
            return Ok(None);
        }

        let app_bundle_id = self.app.config().identifier.as_str();
        if let Some(bundle_id) = self.clipboard.active_bundle_id() {
            if should_ignore_bundle(&bundle_id, app_bundle_id, &settings.denylist_bundle_ids) {
                return Ok(None);
            }
        }

        let hash = hash_for_payload(&payload)?;
        let latest = self.db.latest_clip()?;
        if is_duplicate(latest.as_ref(), &payload, &hash) {
            return Ok(None);
        }

        let clip = match payload {
            ClipboardPayload::Text(content) => {
                let content_type = classify_content_type(&content);
                self.db.insert_text_clip(&content, content_type, &hash)?
            }
            ClipboardPayload::Image(image) => {
                let stored = self.media_store.store_image(&image)?;
                let summary = format_image_summary(&image, &stored);
                self.db.insert_image_clip(ImageClipInsert {
                    content: &summary,
                    hash: &hash,
                    media_path: &stored.media_path,
                    thumb_path: &stored.thumb_path,
                    mime_type: &stored.mime_type,
                    byte_size: stored.byte_size,
                    pixel_width: stored.pixel_width,
                    pixel_height: stored.pixel_height,
                })?
            }
        };

        let pruned = run_prune(&self.db, settings.history_limit)?;
        for pruned_clip in pruned {
            if let Err(err) = self.cleanup_clip_media(&pruned_clip) {
                warn!("failed to clean media for pruned clip {}: {err}", pruned_clip.id);
            }
        }

        let _ = self.app.emit("clips://created", clip.clone());
        Ok(Some(clip))
    }

    pub fn copy_clip(&self, id: i64) -> AppResult<()> {
        let clip = self.db.get_clip(id)?.ok_or(AppError::NotFound)?;

        let (clipboard_payload, pending_payload) = if clip.content_type == "image" {
            let media_path = clip
                .media_path
                .as_ref()
                .ok_or_else(|| AppError::Internal("image clip is missing media path".to_string()))?;
            let bytes = fs::read(media_path).map_err(|err| AppError::Internal(err.to_string()))?;
            let hash = MediaStore::canonical_hash_for_image_bytes(&bytes)?;
            (
                ClipboardPayload::Image(ImagePayload {
                    bytes,
                    mime: clip
                        .mime_type
                        .clone()
                        .unwrap_or_else(|| "image/png".to_string()),
                    format: format_from_mime(clip.mime_type.as_deref()),
                    width: clip.pixel_width.unwrap_or_default() as u32,
                    height: clip.pixel_height.unwrap_or_default() as u32,
                }),
                PendingInternalPayload::ImageHash(hash),
            )
        } else {
            (
                ClipboardPayload::Text(clip.content.clone()),
                PendingInternalPayload::Text(clip.content),
            )
        };

        self.clipboard.set_payload(&clipboard_payload)?;

        let mut pending = self
            .pending_internal_copy
            .lock()
            .map_err(|_| AppError::Internal("pending copy lock poisoned".to_string()))?;
        *pending = Some(PendingInternalCopy {
            payload: pending_payload,
            created_at: Instant::now(),
        });
        Ok(())
    }

    pub fn reconcile_recent_image_duplicates(&self, limit: i64) -> AppResult<usize> {
        let images = self.db.list_image_clips_desc(limit)?;
        if images.is_empty() {
            return Ok(0);
        }

        let mut last_canonical_hash: Option<String> = None;
        let mut duplicate_ids = Vec::new();

        for clip in &images {
            let Some(media_path) = clip.media_path.as_deref() else {
                continue;
            };

            let canonical = match self.media_store.canonical_hash_from_path(media_path) {
                Ok(value) => value,
                Err(err) => {
                    warn!("failed to canonicalize image clip {}: {err}", clip.id);
                    continue;
                }
            };

            if last_canonical_hash
                .as_deref()
                .map(|prev| prev == canonical)
                .unwrap_or(false)
            {
                duplicate_ids.push(clip.id);
                continue;
            }

            last_canonical_hash = Some(canonical);
        }

        if duplicate_ids.is_empty() {
            return Ok(0);
        }

        let deleted = self.db.delete_clips_by_ids(&duplicate_ids)?;
        self.cleanup_media_for_clips(&deleted)?;
        Ok(deleted.len())
    }

    pub fn cleanup_clip_media(&self, clip: &Clip) -> AppResult<()> {
        if clip.content_type != "image" {
            return Ok(());
        }
        self.media_store
            .delete_files_for_clip(clip.media_path.as_deref(), clip.thumb_path.as_deref())
    }

    pub fn cleanup_media_for_clips(&self, clips: &[Clip]) -> AppResult<()> {
        for clip in clips {
            self.cleanup_clip_media(clip)?;
        }
        Ok(())
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    fn should_skip_pending_internal_copy(&self, payload: &ClipboardPayload) -> AppResult<bool> {
        let mut pending = self
            .pending_internal_copy
            .lock()
            .map_err(|_| AppError::Internal("pending copy lock poisoned".to_string()))?;
        let now = Instant::now();

        if should_skip_internal_copy(
            pending.as_ref(),
            payload,
            now,
            INTERNAL_COPY_SUPPRESS_WINDOW,
        ) {
            *pending = None;
            return Ok(true);
        }

        if pending
            .as_ref()
            .map(|entry| now.duration_since(entry.created_at) > INTERNAL_COPY_SUPPRESS_WINDOW)
            .unwrap_or(false)
        {
            *pending = None;
        }

        Ok(false)
    }
}

pub fn should_skip_payload(payload: &ClipboardPayload, max_clip_bytes: i64) -> bool {
    match payload {
        ClipboardPayload::Text(content) => {
            content.trim().is_empty() || content.len() as i64 > max_clip_bytes
        }
        ClipboardPayload::Image(image) => {
            image.bytes.is_empty() || image.bytes.len() as i64 > max_clip_bytes
        }
    }
}

pub fn should_ignore_bundle(bundle_id: &str, app_bundle_id: &str, denylist: &[String]) -> bool {
    bundle_id == app_bundle_id || denylist.iter().any(|item| item == bundle_id)
}

pub fn is_duplicate(latest: Option<&LatestClip>, payload: &ClipboardPayload, hash: &str) -> bool {
    match payload {
        ClipboardPayload::Text(content) => latest
            .map(|entry| {
                entry.content_type != "image" && entry.content == *content && entry.hash == hash
            })
            .unwrap_or(false),
        ClipboardPayload::Image(_) => latest
            .map(|entry| entry.content_type == "image" && entry.hash == hash)
            .unwrap_or(false),
    }
}

pub fn classify_content_type(content: &str) -> &'static str {
    let lower = content.trim().to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return "url";
    }

    let code_signals = [
        "fn ",
        "const ",
        "let ",
        "class ",
        "import ",
        "#include",
        "public ",
        "private ",
        "=>",
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

fn hash_for_payload(payload: &ClipboardPayload) -> AppResult<String> {
    match payload {
        ClipboardPayload::Text(content) => Ok(sha256_hex(content)),
        ClipboardPayload::Image(image) => MediaStore::canonical_hash_for_image_bytes(&image.bytes),
    }
}

fn should_skip_internal_copy(
    pending: Option<&PendingInternalCopy>,
    incoming_payload: &ClipboardPayload,
    now: Instant,
    suppress_window: Duration,
) -> bool {
    pending
        .map(|entry| {
            if now.duration_since(entry.created_at) > suppress_window {
                return false;
            }
            match (&entry.payload, incoming_payload) {
                (PendingInternalPayload::Text(existing), ClipboardPayload::Text(incoming)) => {
                    existing == incoming
                }
                (
                    PendingInternalPayload::ImageHash(existing_hash),
                    ClipboardPayload::Image(incoming),
                ) => MediaStore::canonical_hash_for_image_bytes(&incoming.bytes)
                    .map(|incoming_hash| existing_hash == &incoming_hash)
                    .unwrap_or(false),
                _ => false,
            }
        })
        .unwrap_or(false)
}

fn format_from_mime(mime: Option<&str>) -> String {
    match mime.unwrap_or("image/png") {
        "image/jpeg" => "jpeg".to_string(),
        "image/tiff" => "tiff".to_string(),
        "image/webp" => "webp".to_string(),
        _ => "png".to_string(),
    }
}

fn format_image_summary(image: &ImagePayload, stored: &StoredImage) -> String {
    let size_mb = stored.byte_size as f64 / (1024.0 * 1024.0);
    format!(
        "Image | {} | {}x{} | {:.1} MB",
        image.format.to_ascii_uppercase(),
        stored.pixel_width,
        stored.pixel_height,
        size_mb
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat, RgbaImage};

    use super::*;

    fn image_payload_with_len(len: usize) -> ClipboardPayload {
        ClipboardPayload::Image(ImagePayload {
            bytes: vec![1; len],
            mime: "image/png".to_string(),
            format: "png".to_string(),
            width: 10,
            height: 10,
        })
    }

    fn encoded_image_payload(format: ImageFormat) -> ClipboardPayload {
        let rgba =
            RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]).expect("image");
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba)
            .write_to(&mut output, format)
            .expect("encode");
        ClipboardPayload::Image(ImagePayload {
            bytes: output.into_inner(),
            mime: "image/png".to_string(),
            format: "png".to_string(),
            width: 2,
            height: 1,
        })
    }

    #[test]
    fn classifies_url() {
        assert_eq!(classify_content_type("https://example.com"), "url");
    }

    #[test]
    fn classifies_code() {
        assert_eq!(
            classify_content_type("fn main() { println!(\"x\"); }"),
            "code"
        );
    }

    #[test]
    fn skips_empty_and_oversized_text() {
        assert!(should_skip_payload(&ClipboardPayload::Text("   ".to_string()), 100));
        assert!(should_skip_payload(
            &ClipboardPayload::Text("a".repeat(20)),
            10
        ));
        assert!(!should_skip_payload(
            &ClipboardPayload::Text("hello".to_string()),
            100
        ));
    }

    #[test]
    fn skips_oversized_image_payload() {
        assert!(should_skip_payload(&image_payload_with_len(11), 10));
        assert!(!should_skip_payload(&image_payload_with_len(10), 10));
    }

    #[test]
    fn duplicate_check_matches_latest_text() {
        let latest = LatestClip {
            content: "hello".to_string(),
            content_type: "text".to_string(),
            hash: "abc".to_string(),
        };
        assert!(is_duplicate(
            Some(&latest),
            &ClipboardPayload::Text("hello".to_string()),
            "abc"
        ));
        assert!(!is_duplicate(
            Some(&latest),
            &ClipboardPayload::Text("hello!".to_string()),
            "abc"
        ));
    }

    #[test]
    fn duplicate_check_matches_latest_image_hash() {
        let payload = encoded_image_payload(ImageFormat::Png);
        let hash = hash_for_payload(&payload).expect("hash");
        let latest = LatestClip {
            content: "Image".to_string(),
            content_type: "image".to_string(),
            hash: hash.clone(),
        };
        assert!(is_duplicate(Some(&latest), &payload, &hash));
    }

    #[test]
    fn canonical_image_hash_is_format_independent() {
        let png_payload = encoded_image_payload(ImageFormat::Png);
        let tiff_payload = encoded_image_payload(ImageFormat::Tiff);
        let png_hash = hash_for_payload(&png_payload).expect("png hash");
        let tiff_hash = hash_for_payload(&tiff_payload).expect("tiff hash");
        assert_eq!(png_hash, tiff_hash);
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

    #[test]
    fn skips_pending_internal_copy_within_suppress_window() {
        let pending = PendingInternalCopy {
            payload: PendingInternalPayload::Text("copy me".to_string()),
            created_at: Instant::now(),
        };

        assert!(should_skip_internal_copy(
            Some(&pending),
            &ClipboardPayload::Text("copy me".to_string()),
            Instant::now(),
            Duration::from_secs(2)
        ));
    }

    #[test]
    fn does_not_skip_pending_internal_copy_if_content_differs() {
        let pending = PendingInternalCopy {
            payload: PendingInternalPayload::Text("copy me".to_string()),
            created_at: Instant::now(),
        };

        assert!(!should_skip_internal_copy(
            Some(&pending),
            &ClipboardPayload::Text("different".to_string()),
            Instant::now(),
            Duration::from_secs(2)
        ));
    }

    #[test]
    fn does_not_skip_pending_internal_copy_after_window_expires() {
        let pending = PendingInternalCopy {
            payload: PendingInternalPayload::Text("copy me".to_string()),
            created_at: Instant::now() - Duration::from_secs(3),
        };

        assert!(!should_skip_internal_copy(
            Some(&pending),
            &ClipboardPayload::Text("copy me".to_string()),
            Instant::now(),
            Duration::from_secs(2)
        ));
    }

    #[test]
    fn image_pending_copy_uses_hash_match() {
        let payload = encoded_image_payload(ImageFormat::Png);
        let pending = PendingInternalCopy {
            payload: PendingInternalPayload::ImageHash(hash_for_payload(&payload).expect("hash")),
            created_at: Instant::now(),
        };

        assert!(should_skip_internal_copy(
            Some(&pending),
            &payload,
            Instant::now(),
            Duration::from_secs(2)
        ));
    }
}
