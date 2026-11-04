#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

mod adapters;
mod api;
mod config;
mod container;
mod db;
mod models;
mod ports;
mod router;
mod services;
mod validation;
mod agent;
mod tools;

use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

static QUITTING: AtomicBool = AtomicBool::new(false);

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().config_dir().map_err(|error| error.to_string())?;
    Ok(base.join("Heimdall"))
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Heimdall")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_window(app),
            "quit" => {
                QUITTING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

#[tauri::command]
async fn api_call(
    router: tauri::State<'_, Arc<router::Router>>,
    method: String,
    path: String,
    body: Option<Value>,
) -> Result<Value, String> {
    let router = router.inner().clone();
    Ok(router.dispatch(&method, &path, body.unwrap_or(Value::Null)).await)
}

#[tauri::command]
fn window_close(window: tauri::WebviewWindow) {
    let _ = window.close();
}

#[tauri::command]
fn save_chart_png(app: AppHandle, name: String, data: Vec<u8>) -> Result<String, String> {
    let dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().document_dir())
        .map_err(|error| error.to_string())?;
    let stem: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(60)
        .collect();
    let stem = if stem.is_empty() { "chart".to_string() } else { stem };
    let mut path = dir.join(format!("{}.png", stem));
    let mut counter = 1;
    while path.exists() {
        path = dir.join(format!("{}-{}.png", stem, counter));
        counter += 1;
    }
    std::fs::write(&path, data).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn window_toggle_maximize(window: tauri::WebviewWindow) -> bool {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
    window.is_maximized().unwrap_or(false)
}

#[tauri::command]
fn window_is_maximized(window: tauri::WebviewWindow) -> bool {
    window.is_maximized().unwrap_or(false)
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![api_call, window_close, window_toggle_maximize, window_is_maximized, save_chart_png])
        .setup(|app| {
            let handle = app.handle().clone();
            let dir = data_dir(&handle)?;
            let database = db::Db::open(&dir).map_err(|error| error.to_string())?;
            let container = Arc::new(container::build_container(database, &dir).map_err(|error| error.to_string())?);
            let router = Arc::new(api::build_router(container.clone()));
            let stop = Arc::new(AtomicBool::new(false));
            tauri::async_runtime::spawn(services::sync::run_periodic(container.clone(), stop.clone()));
            tauri::async_runtime::spawn(services::scheduler::run_scheduler(container.clone(), stop.clone()));
            app.manage(container);
            app.manage(router);
            app.manage(stop);
            create_tray(&handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Heimdall");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, code, .. } => {
            if code.is_none() && !QUITTING.load(Ordering::SeqCst) {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            QUITTING.store(true, Ordering::SeqCst);
            if let Some(stop) = app.try_state::<Arc<AtomicBool>>() {
                stop.store(true, Ordering::SeqCst);
            }
            if let Some(container) = app.try_state::<Arc<container::Container>>() {
                container.wake.set();
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_window(app),
        _ => {}
    });
}
