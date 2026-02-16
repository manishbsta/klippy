use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::db::ClipPage;
use crate::services::clip_engine::ClipEngine;

pub struct AppState {
    pub engine: Arc<ClipEngine>,
}

#[derive(Clone, Debug, Serialize)]
struct DeletedPayload {
    id: i64,
}

#[tauri::command]
pub fn list_clips(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<ClipPage, String> {
    state
        .engine
        .db()
        .list_clips(query.as_deref(), limit, offset)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn copy_clip(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.engine.copy_clip(id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    pinned: bool,
) -> Result<(), String> {
    let clip = state
        .engine
        .db()
        .set_pinned(id, pinned)
        .map_err(|err| err.to_string())?;

    if let Some(clip) = clip {
        let _ = app.emit("clips://updated", clip);
        Ok(())
    } else {
        Err("clip not found".to_string())
    }
}

#[tauri::command]
pub fn delete_clip(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let deleted = state
        .engine
        .db()
        .delete_clip(id)
        .map_err(|err| err.to_string())?;

    if !deleted {
        return Err("clip not found".to_string());
    }

    let _ = app.emit("clips://deleted", DeletedPayload { id });
    Ok(())
}

#[tauri::command]
pub fn clear_all_clips(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    let deleted = state
        .engine
        .db()
        .delete_all_clips()
        .map_err(|err| err.to_string())?;
    let _ = app.emit("clips://updated", true);
    Ok(deleted)
}

#[tauri::command]
pub fn stop_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
