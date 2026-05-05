// Modules
mod commands;
mod config;
mod process;
mod runtime;

// Re-exports
pub use process::manager::ProcessManager;
pub use process::{ServiceInfo, ServiceMap, ServiceState, ServiceType};

use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconId};
use tauri::{menu::MenuEvent, AppHandle, Emitter, Manager};

// Global state for the process manager
pub struct AppState {
    pub process_manager: Mutex<process::manager::ProcessManager>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            process_manager: Mutex::new(process::manager::ProcessManager::new()),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .on_menu_event(handle_menu_event)
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                use tauri::menu::{Menu, MenuItem, Submenu};

                // Create debug menu items with IDs
                let open_download_folder = MenuItem::with_id(
                    app,
                    "open-download-folder",
                    "View Download Folder (ZIP files)",
                    true,
                    None::<&str>,
                )?;
                let open_runtime_folder = MenuItem::with_id(
                    app,
                    "open-runtime-folder",
                    "Open Runtime Folder",
                    true,
                    None::<&str>,
                )?;
                let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
                let reset_installation = MenuItem::with_id(
                    app,
                    "reset-installation",
                    "Reset Installation",
                    true,
                    None::<&str>,
                )?;
                let show_wizard = MenuItem::with_id(
                    app,
                    "show-wizard",
                    "Show First-Run Wizard",
                    true,
                    None::<&str>,
                )?;

                // Create submenu with items
                let debug_menu = Submenu::with_items(
                    app,
                    "Debug",
                    true,
                    &[
                        &open_download_folder,
                        &open_runtime_folder,
                        &separator,
                        &reset_installation,
                        &show_wizard,
                    ],
                )?;
                let menu = Menu::with_items(app, &[&debug_menu])?;
                app.set_menu(menu)?;
            }

            // Setup system tray
            setup_system_tray(app)?;

            if crate::config::AppSettings::load().auto_start_services {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        if let Ok(mut manager) = state.process_manager.lock() {
                            let _ = manager.start_all();
                        }
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Handle window close event - minimize to tray instead of closing
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Service management commands
            commands::start_service,
            commands::stop_service,
            commands::restart_service,
            commands::start_all_services,
            commands::stop_all_services,
            commands::restart_all_services,
            commands::get_all_statuses,
            // Settings commands
            commands::get_settings,
            commands::save_settings,
            commands::validate_settings,
            commands::check_ports,
            // Runtime download commands
            commands::check_runtime_installed,
            commands::check_system_dependencies,
            commands::download_runtime,
            commands::download_runtime_with_packages,
            commands::download_runtime_with_skip,
            commands::get_available_packages_cmd,
            commands::get_package_selection,
            commands::update_package_selection,
            commands::get_installed_php_versions,
            commands::switch_php_version,
            commands::download_php_version,
            commands::get_selected_package_ids,
            commands::reload_runtime_config,
            commands::get_installed_versions,
            commands::check_existing_components,
            commands::get_runtime_dir,
            commands::get_app_paths,
            commands::get_download_dir,
            commands::get_install_dir,
            commands::open_folder,
            commands::open_manual,
            commands::reset_installation,
            commands::cleanup_all_services,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        // Debug menu items
        "open-download-folder" => {
            tauri::async_runtime::spawn(async move {
                if let Ok(download_dir) = commands::get_download_dir().await {
                    let _ = commands::open_folder(download_dir).await;
                }
            });
        }
        "open-runtime-folder" => {
            tauri::async_runtime::spawn(async move {
                if let Ok(runtime_dir) = commands::get_runtime_dir().await {
                    let _ = commands::open_folder(runtime_dir).await;
                }
            });
        }
        "reset-installation" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(_) = commands::reset_installation().await {
                    let _ = app.emit("show-wizard", ());
                }
            });
        }
        "show-wizard" => {
            let _ = app.emit("show-wizard", ());
        }
        // Tray menu items
        "tray-show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "tray-hide" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        "tray-quit" => {
            // Cleanup services before quitting
            if let Some(state) = app.try_state::<AppState>() {
                let _ = state.process_manager.lock().unwrap().stop_all();
            }
            std::process::exit(0);
        }
        _ => {}
    }
}

fn setup_system_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Embed the icon directly in the binary to ensure it's always available
    #[cfg(target_os = "windows")]
    let icon_bytes = include_bytes!("../icons/icon.ico");
    #[cfg(not(target_os = "windows"))]
    let icon_bytes = include_bytes!("../icons/32x32.png");

    // Load and decode the embedded image
    let img = image::load_from_memory(icon_bytes)?;
    let rgba = img.to_rgba8();
    let dimensions = rgba.dimensions();
    let raw_bytes = rgba.as_raw().to_vec();

    // Create Tauri Image from raw RGBA bytes
    let tray_icon = Image::new_owned(raw_bytes, dimensions.0, dimensions.1);

    // Create tray menu items
    let show_item = MenuItem::with_id(app, "tray-show", "Show CHAMP", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "tray-hide", "Hide to Tray", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "tray-quit", "Quit CHAMP", true, None::<&str>)?;

    // Create tray menu
    let menu = Menu::with_items(app, &[&show_item, &hide_item, &separator, &quit_item])?;

    // Build and set tray icon
    let tray_id = TrayIconId::new("main-tray");
    TrayIconBuilder::with_id(tray_id)
        .menu(&menu)
        .tooltip("CHAMP - Local Web Stack")
        .icon(tray_icon)
        .build(app)?;

    Ok(())
}
