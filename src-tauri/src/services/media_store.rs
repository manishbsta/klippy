use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::ImageFormat;

use crate::clipboard::ImagePayload;
use crate::error::{AppError, AppResult};
use crate::utils::hash::sha256_hex_bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredImage {
    pub media_path: String,
    pub thumb_path: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub pixel_width: i64,
    pub pixel_height: i64,
}

pub struct MediaStore {
    originals_dir: PathBuf,
    thumbs_dir: PathBuf,
}

impl MediaStore {
    pub fn new(root_dir: &Path) -> AppResult<Self> {
        let originals_dir = root_dir.join("originals");
        let thumbs_dir = root_dir.join("thumbs");
        fs::create_dir_all(&originals_dir).map_err(to_internal)?;
        fs::create_dir_all(&thumbs_dir).map_err(to_internal)?;
        Ok(Self {
            originals_dir,
            thumbs_dir,
        })
    }

    pub fn store_image(&self, payload: &ImagePayload) -> AppResult<StoredImage> {
        let digest = sha256_hex_bytes(&payload.bytes);
        let extension = extension_for_format(&payload.format);
        let media_path = self.originals_dir.join(format!("{digest}.{extension}"));
        let thumb_path = self.thumbs_dir.join(format!("{digest}.png"));

        if !media_path.exists() {
            fs::write(&media_path, &payload.bytes).map_err(to_internal)?;
        }

        if !thumb_path.exists() {
            let decoded = image::load_from_memory(&payload.bytes)
                .map_err(|err| AppError::Internal(format!("failed to decode image: {err}")))?;
            let thumbnail = decoded.thumbnail(96, 96);
            let mut output = Cursor::new(Vec::new());
            thumbnail
                .write_to(&mut output, ImageFormat::Png)
                .map_err(|err| AppError::Internal(format!("failed to encode thumbnail: {err}")))?;
            fs::write(&thumb_path, output.into_inner()).map_err(to_internal)?;
        }

        Ok(StoredImage {
            media_path: media_path.to_string_lossy().to_string(),
            thumb_path: thumb_path.to_string_lossy().to_string(),
            mime_type: payload.mime.clone(),
            byte_size: payload.bytes.len() as i64,
            pixel_width: payload.width as i64,
            pixel_height: payload.height as i64,
        })
    }

    pub fn delete_files_for_clip(
        &self,
        media_path: Option<&str>,
        thumb_path: Option<&str>,
    ) -> AppResult<()> {
        if let Some(path) = media_path {
            remove_file_if_exists(path)?;
        }
        if let Some(path) = thumb_path {
            remove_file_if_exists(path)?;
        }
        Ok(())
    }

    pub fn cleanup_orphans(&self, referenced_paths: &HashSet<String>) -> AppResult<()> {
        cleanup_dir_orphans(&self.originals_dir, referenced_paths)?;
        cleanup_dir_orphans(&self.thumbs_dir, referenced_paths)?;
        Ok(())
    }

    pub fn canonical_hash_for_image_bytes(bytes: &[u8]) -> AppResult<String> {
        let decoded = image::load_from_memory(bytes)
            .map_err(|err| AppError::Internal(format!("failed to decode image: {err}")))?;
        let rgba = decoded.to_rgba8();
        Ok(sha256_hex_bytes(rgba.as_raw()))
    }

    pub fn canonical_hash_from_path(&self, media_path: &str) -> AppResult<String> {
        let bytes = fs::read(media_path).map_err(to_internal)?;
        Self::canonical_hash_for_image_bytes(&bytes)
    }
}

fn extension_for_format(format: &str) -> &'static str {
    match format.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => "jpg",
        "tiff" | "tif" => "tiff",
        "webp" => "webp",
        _ => "png",
    }
}

fn cleanup_dir_orphans(dir: &Path, referenced_paths: &HashSet<String>) -> AppResult<()> {
    for entry in fs::read_dir(dir).map_err(to_internal)? {
        let entry = entry.map_err(to_internal)?;
        let path = entry.path();
        let path_string = path.to_string_lossy().to_string();
        if !referenced_paths.contains(&path_string) {
            remove_file_if_exists(path_string.as_str())?;
        }
    }
    Ok(())
}

fn remove_file_if_exists(path: &str) -> AppResult<()> {
    let path = Path::new(path);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).map_err(to_internal)?;
    Ok(())
}

fn to_internal(err: std::io::Error) -> AppError {
    AppError::Internal(err.to_string())
}
