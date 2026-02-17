mod clipboard;
mod commands;
mod db;
mod error;
mod services;
mod utils;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use commands::AppState;
use services::clip_engine::ClipEngine;
use services::media_store::MediaStore;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartManagerExt};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::level_filters::LevelFilter;
use tracing::warn;

const FIXED_WINDOW_WIDTH: f64 = 560.0;

#[derive(Default)]
struct WindowPlacementState {
    suppress_next_move_event: AtomicBool,
    suppress_next_resize_event: AtomicBool,
}

fn place_window_top_right(window: &WebviewWindow, placement: &WindowPlacementState) {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        return;
    };

    let scale = monitor.scale_factor().max(1.0);
    let monitor_pos_x = monitor.position().x as f64 / scale;
    let monitor_pos_y = monitor.position().y as f64 / scale;
    let monitor_width = monitor.size().width as f64 / scale;
    let window_width = window
        .outer_size()
        .map(|size| size.width as f64 / scale)
        .unwrap_or(600.0);
    let margin = 14.0;

    let x = monitor_pos_x + monitor_width - window_width - margin;
    let y = monitor_pos_y + margin;

    placement
        .suppress_next_move_event
        .store(true, Ordering::SeqCst);
    let _ = window.set_position(LogicalPosition::new(x, y));
}

fn set_window_size(
    window: &WebviewWindow,
    placement: &WindowPlacementState,
    width: f64,
    height: f64,
) {
    placement
        .suppress_next_resize_event
        .store(true, Ordering::SeqCst);
    let _ = window.set_size(LogicalSize::new(width, height));
}

fn toggle_window(app: &AppHandle, window: &WebviewWindow) {
    let is_visible: bool = window.is_visible().unwrap_or_default();
    let is_minimized: bool = window.is_minimized().unwrap_or_default();

    if should_hide_on_focus_loss(is_visible, is_minimized) {
        let _ = window.hide();
        return;
    }

    let placement = app.state::<WindowPlacementState>();
    place_window_top_right(window, placement.inner());

    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn should_hide_on_focus_loss(is_visible: bool, is_minimized: bool) -> bool {
    is_visible && !is_minimized
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .with_target(false)
        .compact()
        .init();

    let app_builder = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None::<Vec<&'static str>>,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    if let Some(window) = app.get_webview_window("main") {
                        toggle_window(app, &window);
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(WindowPlacementState::default());

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_minimizable(false);
                let monitor = window
                    .current_monitor()
                    .ok()
                    .flatten()
                    .or_else(|| window.primary_monitor().ok().flatten());
                if let Some(monitor) = monitor {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let monitor_height = (monitor.size().height as f64 / scale).max(1.0);
                    let min_height = (monitor_height * 0.80).round();
                    let _ =
                        window.set_min_size(Some(LogicalSize::new(FIXED_WINDOW_WIDTH, min_height)));
                    let _ = window
                        .set_max_size(Some(LogicalSize::new(FIXED_WINDOW_WIDTH, monitor_height)));

                    if let Ok(inner_size) = window.inner_size() {
                        let current_height = inner_size.height as f64 / scale;
                        let next_height = current_height.max(min_height);
                        if current_height < min_height
                            || (inner_size.width as f64 / scale - FIXED_WINDOW_WIDTH).abs() > 0.5
                        {
                            let placement = app.state::<WindowPlacementState>();
                            set_window_size(
                                &window,
                                placement.inner(),
                                FIXED_WINDOW_WIDTH,
                                next_height,
                            );
                        }
                    }
                }

                let placement = app.state::<WindowPlacementState>();
                place_window_top_right(&window, placement.inner());
            }

            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|err| crate::error::AppError::Internal(err.to_string()).to_string())?;
            std::fs::create_dir_all(&app_data_dir)
                .map_err(|err| crate::error::AppError::Internal(err.to_string()).to_string())?;

            let db_path = app_data_dir.join("klippy.sqlite3");
            let db = Arc::new(db::Database::new(&db_path).map_err(|err| err.to_string())?);
            let media_store = Arc::new(
                MediaStore::new(&app_data_dir.join("media")).map_err(|err| err.to_string())?,
            );
            let referenced = db.referenced_media_paths().map_err(|err| err.to_string())?;
            media_store
                .cleanup_orphans(&referenced)
                .map_err(|err| err.to_string())?;
            let clipboard = clipboard::default_service();
            let engine = Arc::new(ClipEngine::new(
                db.clone(),
                clipboard,
                media_store,
                app.handle().clone(),
            ));
            engine.start().map_err(|err| err.to_string())?;

            let settings = db.get_settings().map_err(|err| err.to_string())?;
            let pruned = db.prune_excess(settings.history_limit).map_err(|err| err.to_string())?;
            engine
                .cleanup_media_for_clips(&pruned)
                .map_err(|err| err.to_string())?;
            if let Err(err) = engine.reconcile_recent_image_duplicates(500) {
                warn!("failed to reconcile image duplicates: {err}");
            }

            let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV);
            app.global_shortcut()
                .register(shortcut)
                .map_err(|err| err.to_string())?;

            if let Err(err) = app.autolaunch().enable() {
                warn!("failed to enable autostart: {err}");
            }

            if let Some(icon) = app.default_window_icon().cloned() {
                let _tray = tauri::tray::TrayIconBuilder::with_id("klippy-tray")
                    .icon(icon)
                    .show_menu_on_left_click(false)
                    .on_tray_icon_event(|tray, event| {
                        if let tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        } = event
                        {
                            if let Some(window) = tray.app_handle().get_webview_window("main") {
                                let app_handle = tray.app_handle();
                                toggle_window(app_handle, &window);
                            }
                        }
                    })
                    .build(app)
                    .map_err(|err| err.to_string())?;
            } else {
                warn!("no default window icon available for tray icon");
            }

            app.manage(AppState { engine });

            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) => {
                let is_visible: bool = window.is_visible().unwrap_or_default();
                let is_minimized: bool = window.is_minimized().unwrap_or_default();
                if should_hide_on_focus_loss(is_visible, is_minimized) {
                    let _ = window.hide();
                }
            }
            WindowEvent::Moved(_) => {
                let app_handle = window.app_handle();
                let placement = app_handle.state::<WindowPlacementState>();
                if placement
                    .suppress_next_move_event
                    .swap(false, Ordering::SeqCst)
                {
                    return;
                }
                if let Some(main_window) = app_handle.get_webview_window(window.label()) {
                    place_window_top_right(&main_window, placement.inner());
                }
            }
            WindowEvent::Resized(size) => {
                let app_handle = window.app_handle();
                let placement = app_handle.state::<WindowPlacementState>();
                if placement
                    .suppress_next_resize_event
                    .swap(false, Ordering::SeqCst)
                {
                    return;
                }

                let scale = window.scale_factor().unwrap_or(1.0);
                let current_width = size.width as f64 / scale;
                if (current_width - FIXED_WINDOW_WIDTH).abs() > 0.5 {
                    let current_height = size.height as f64 / scale;
                    if let Some(main_window) = app_handle.get_webview_window(window.label()) {
                        set_window_size(
                            &main_window,
                            placement.inner(),
                            FIXED_WINDOW_WIDTH,
                            current_height,
                        );
                    }
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_clips,
            commands::copy_clip,
            commands::set_pinned,
            commands::delete_clip,
            commands::clear_all_clips,
            commands::stop_app
        ]);

    if let Err(err) = app_builder.run(tauri::generate_context!()) {
        eprintln!("error while running tauri application: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::should_hide_on_focus_loss;

    #[test]
    fn hides_only_when_window_is_visible_and_not_minimized() {
        assert!(should_hide_on_focus_loss(true, false));
        assert!(!should_hide_on_focus_loss(false, false));
        assert!(!should_hide_on_focus_loss(true, true));
        assert!(!should_hide_on_focus_loss(false, true));
    }
}
