// Modules
mod agent;
mod commands;
mod config;
mod constants;
mod doctor;
mod fs_util;
mod logs;
mod metrics;
mod process;
mod runtime;
mod tunnel;

// Re-exports
pub use config::AppSettings;
pub use process::manager::ProcessManager;
pub use process::{ServiceInfo, ServiceMap, ServiceState, ServiceType};

use std::sync::atomic::{AtomicBool, Ordering};
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

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new())
        .on_menu_event(handle_menu_event)
        .setup(|app| {
            #[cfg(debug_assertions)]
            build_debug_menu(app)?;

            // Setup system tray
            setup_system_tray(app)?;

            match app.path().resource_dir() {
                Ok(resource_dir) => crate::runtime::packages::set_tauri_resource_dir(resource_dir),
                Err(e) => eprintln!("Warning: failed to get Tauri resource dir: {e}"),
            }

            if crate::config::AppSettings::load().agent_connector_enabled {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    agent::apply(&app_handle).await;
                });
            }

            doctor::spawn_default_model_download(app.handle().clone());
            tunnel::spawn_stale_agent_cleanup();

            if crate::config::AppSettings::load().auto_start_services {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = process::runner::run_with_manager(&app_handle, |manager| {
                        manager.start_all()
                    })
                    .await;
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window keeps CHAMP available from the system tray.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Service management commands
            commands::services::start_service,
            commands::services::stop_service,
            commands::services::restart_service,
            commands::services::start_all_services,
            commands::services::stop_all_services,
            commands::services::restart_all_services,
            commands::services::get_all_statuses,
            commands::services::start_https_tunnel,
            commands::services::stop_https_tunnel,
            commands::services::get_https_tunnel_status,
            commands::services::set_ngrok_authtoken,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_language_settings,
            commands::settings::save_language_setting,
            commands::settings::save_sound_setting,
            commands::settings::validate_settings,
            commands::settings::check_ports,
            // Runtime download commands
            commands::runtime::check_runtime_installed,
            commands::runtime::check_system_dependencies,
            commands::runtime::download_runtime_with_packages,
            commands::runtime::download_runtime_with_skip,
            commands::database::switch_database_engine,
            commands::database::switch_database_tool,
            commands::runtime::get_available_packages_cmd,
            commands::runtime::refresh_runtime_catalog,
            commands::runtime::refresh_php_catalog,
            commands::runtime::get_runtime_platform,
            commands::runtime::update_package_selection,
            commands::php::get_installed_php_versions,
            commands::php::switch_php_version,
            commands::php::download_php_version,
            commands::runtime::get_selected_package_ids,
            commands::runtime::get_installed_versions,
            commands::runtime::check_existing_components,
            commands::folders::get_runtime_dir,
            commands::folders::get_app_paths,
            commands::services::get_system_metrics,
            commands::folders::get_download_dir,
            commands::folders::open_folder,
            commands::folders::open_project_terminal,
            commands::runtime::reset_installation,
            commands::services::prepare_for_update,
            // AI agent connector (MCP)
            agent::get_agent_connector_status,
            agent::configure_agent_connector,
            agent::regenerate_agent_token,
            agent::test_agent_connector,
            // Log Doctor
            doctor::diagnose_service,
            doctor::get_log_doctor_status,
            doctor::download_log_doctor_model,
            doctor::remove_log_doctor_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(debug_assertions)]
fn build_debug_menu(app: &tauri::App) -> tauri::Result<()> {
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
    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        // Debug menu items
        "open-download-folder" => {
            tauri::async_runtime::spawn(async move {
                if let Ok(download_dir) = commands::folders::get_download_dir().await {
                    let _ = commands::folders::open_folder(download_dir).await;
                }
            });
        }
        "open-runtime-folder" => {
            tauri::async_runtime::spawn(async move {
                if let Ok(runtime_dir) = commands::folders::get_runtime_dir().await {
                    let _ = commands::folders::open_folder(runtime_dir).await;
                }
            });
        }
        "reset-installation" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = commands::services::stop_tunnel_and_services(&app).await;
                if commands::runtime::reset_runtime_dir().await.is_ok() {
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
            quit_app(app);
        }
        _ => {}
    }
}

/// Shut everything down off the main thread, then exit.
///
/// The manager lock can be held for minutes during a database initialization,
/// so waiting for it inside the menu callback would freeze the UI. The public
/// tunnel goes first: on macOS and Linux nothing else would stop it.
fn quit_app(app: &AppHandle) {
    static QUITTING: AtomicBool = AtomicBool::new(false);
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        agent::stop();
        let _ = tunnel::stop_https_tunnel();
        let _ = process::runner::run_with_manager(&app, |manager| manager.stop_all()).await;
        app.exit(0);
    });
}

fn setup_system_tray(app: &tauri::App) -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Embed a tray-sized icon in the binary so it is always available. 64px
    // stays sharp on HiDPI trays without shipping the 190 KB master icon.
    let tray_icon = match Image::from_bytes(include_bytes!("../icons/64x64.png")) {
        Ok(icon) => icon,
        Err(e) => {
            eprintln!("Warning: failed to decode tray icon: {e}. Using blank icon.");
            // 1×1 transparent RGBA fallback so the app still starts
            Image::new_owned(vec![0u8; 4], 1, 1)
        }
    };

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
