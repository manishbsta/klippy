mod clipboard;
mod commands;
mod db;
mod error;
mod services;
mod utils;

use std::sync::Arc;

use commands::AppState;
use services::clip_engine::ClipEngine;
use tauri::{LogicalSize, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartManagerExt};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::level_filters::LevelFilter;
use tracing::warn;

fn toggle_window(window: &WebviewWindow) {
    let is_visible: bool = window.is_visible().unwrap_or_default();
    let is_minimized: bool = window.is_minimized().unwrap_or_default();

    if is_visible && !is_minimized {
        let _ = window.minimize();
        return;
    }

    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
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
                        toggle_window(&window);
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);

            if let Some(window) = app.get_webview_window("main") {
                let monitor = window
                    .current_monitor()
                    .ok()
                    .flatten()
                    .or_else(|| window.primary_monitor().ok().flatten());
                if let Some(monitor) = monitor {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let monitor_width = (monitor.size().width as f64 / scale).max(1.0);
                    let monitor_height = (monitor.size().height as f64 / scale).max(1.0);
                    let min_width = (monitor_width * 0.30).round().max(520.0).min(620.0);
                    let min_height = (monitor_height * 0.80).round();
                    let _ = window.set_min_size(Some(LogicalSize::new(min_width, min_height)));

                    if let Ok(inner_size) = window.inner_size() {
                        let current_width = inner_size.width as f64 / scale;
                        let current_height = inner_size.height as f64 / scale;
                        if current_width < min_width || current_height < min_height {
                            let next_width = current_width.max(min_width);
                            let next_height = current_height.max(min_height);
                            let _ = window.set_size(LogicalSize::new(next_width, next_height));
                        }
                    }
                }
            }

            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|err| crate::error::AppError::Internal(err.to_string()).to_string())?;
            std::fs::create_dir_all(&app_data_dir)
                .map_err(|err| crate::error::AppError::Internal(err.to_string()).to_string())?;

            let db_path = app_data_dir.join("klippy.sqlite3");
            let db = Arc::new(db::Database::new(&db_path).map_err(|err| err.to_string())?);
            let clipboard = clipboard::default_service();
            let engine = Arc::new(ClipEngine::new(db.clone(), clipboard, app.handle().clone()));
            engine.start().map_err(|err| err.to_string())?;

            let settings = db.get_settings().map_err(|err| err.to_string())?;
            let _ = db.prune_excess(settings.history_limit);

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
                                toggle_window(&window);
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
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.minimize();
            }
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
