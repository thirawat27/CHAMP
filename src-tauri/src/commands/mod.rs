//! Tauri IPC Commands
//!
//! This module contains all Tauri commands that are exposed to the frontend.
//! Commands are invoked from the React frontend using the `invoke()` function.
//!
//! # Command Categories
//!
//! - **Folder Operations**: `open_folder`, `open_project_terminal`
//! - **Service Control**: `start_service`, `stop_service`, `restart_service`, `start_all_services`, `stop_all_services`, `restart_all_services`
//! - **Service Status**: `get_all_statuses`
//! - **HTTPS Tunnel**: `start_https_tunnel`, `stop_https_tunnel`, `get_https_tunnel_status`
//! - **Settings**: `get_settings`, `save_settings`, `validate_settings`
//! - **Port Checking**: `check_ports`
//! - **Runtime Management**: `check_runtime_installed`, `download_runtime_with_packages`, `download_runtime_with_skip`
//! - **Installation**: `reset_installation`, `get_runtime_dir`, `get_app_paths`
//! - **System Metrics**: `get_system_metrics`
//! - **Package Management**: `get_available_packages_cmd`, `get_selected_package_ids`, `update_package_selection`
//! - **PHP Version Management**: `get_installed_php_versions`, `switch_php_version`, `download_php_version`
//! - **Version Info**: `get_installed_versions`, `check_existing_components`
//! - **Dependencies**: `check_system_dependencies`
//!
//! Service commands run the blocking process-manager work on Tokio's blocking
//! pool so slow starts (MySQL init, port waits) never stall async IPC workers.

pub mod database;
pub mod folders;
pub mod php;
pub mod runtime;
pub mod services;
pub mod settings;
