use std::borrow::Cow;
use std::io::Cursor;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use arboard::{Clipboard, ImageData};
use image::{DynamicImage, ImageFormat, RgbaImage};

use super::{
    should_emit_change, ClipCallback, ClipboardError, ClipboardPayload, ClipboardService,
    ImagePayload,
};
use crate::utils::hash::{sha256_hex, sha256_hex_bytes};

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

    fn clipboard() -> Result<Clipboard, ClipboardError> {
        Clipboard::new()
            .map_err(|err| ClipboardError::Command(format!("failed to access clipboard: {err}")))
    }

    fn encode_image_payload(image_data: ImageData<'_>) -> Result<ImagePayload, ClipboardError> {
        let width = image_data.width as u32;
        let height = image_data.height as u32;
        let bytes = image_data.bytes.into_owned();
        let rgba = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
            ClipboardError::Command("clipboard image payload was malformed".to_string())
        })?;

        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba)
            .write_to(&mut output, ImageFormat::Png)
            .map_err(|err| ClipboardError::Command(format!("failed to encode image: {err}")))?;

        Ok(ImagePayload {
            bytes: output.into_inner(),
            mime: "image/png".to_string(),
            format: "png".to_string(),
            width,
            height,
        })
    }

    fn read_payload(clipboard: &mut Clipboard) -> Result<Option<ClipboardPayload>, ClipboardError> {
        if let Ok(image_data) = clipboard.get_image() {
            let image = Self::encode_image_payload(image_data)?;
            return Ok(Some(ClipboardPayload::Image(image)));
        }

        if let Ok(text) = clipboard.get_text() {
            return Ok(Some(ClipboardPayload::Text(text)));
        }

        Ok(None)
    }

    fn run_osascript(script: &str) -> Result<String, ClipboardError> {
        let output = Command::new("osascript").arg("-e").arg(script).output()?;
        if !output.status.success() {
            return Err(ClipboardError::Command(
                "osascript exited unsuccessfully".to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn payload_signature(payload: &ClipboardPayload) -> String {
        match payload {
            ClipboardPayload::Text(text) => format!("text:{}", sha256_hex(text)),
            ClipboardPayload::Image(image) => format!("image:{}", sha256_hex_bytes(&image.bytes)),
        }
    }

    fn decode_image_bytes(payload: &ImagePayload) -> Result<(usize, usize, Vec<u8>), ClipboardError> {
        let decoded = image::load_from_memory(&payload.bytes).map_err(|err| {
            ClipboardError::Command(format!("failed to decode image clipboard bytes: {err}"))
        })?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok((width as usize, height as usize, rgba.into_raw()))
    }
}

impl ClipboardService for MacOsClipboard {
    fn set_payload(&self, payload: &ClipboardPayload) -> Result<(), ClipboardError> {
        let mut clipboard = Self::clipboard()?;
        match payload {
            ClipboardPayload::Text(content) => clipboard
                .set_text(content.clone())
                .map_err(|err| ClipboardError::Command(format!("failed to set text: {err}")))?,
            ClipboardPayload::Image(image) => {
                let (width, height, rgba) = Self::decode_image_bytes(image)?;
                clipboard
                    .set_image(ImageData {
                        width,
                        height,
                        bytes: Cow::Owned(rgba),
                    })
                    .map_err(|err| ClipboardError::Command(format!("failed to set image: {err}")))?;
            }
        }
        Ok(())
    }

    fn watch_changes(&self, callback: ClipCallback) -> Result<(), ClipboardError> {
        let poll = self.poll_ms;
        thread::spawn(move || {
            let mut previous_signature: Option<String> = None;
            let mut last_emitted = Instant::now() - Duration::from_millis(DEBOUNCE_MS * 2);
            let debounce = Duration::from_millis(DEBOUNCE_MS);
            let mut clipboard = Self::clipboard().ok();

            loop {
                if clipboard.is_none() {
                    clipboard = Self::clipboard().ok();
                }

                if let Some(handle) = clipboard.as_mut() {
                    let payload = Self::read_payload(handle);
                    match payload {
                        Ok(Some(next)) => {
                            let signature = Self::payload_signature(&next);
                            if should_emit_change(
                                &mut previous_signature,
                                &signature,
                                &mut last_emitted,
                                debounce,
                            ) {
                                callback(next);
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {
                            clipboard = None;
                        }
                    }
                }
                thread::sleep(Duration::from_millis(poll));
            }
        });
        Ok(())
    }

    fn active_bundle_id(&self) -> Option<String> {
        let script = "tell application \"System Events\" to get bundle identifier of first process whose frontmost is true";
        Self::run_osascript(script)
            .ok()
            .filter(|bundle| !bundle.is_empty())
    }
}
