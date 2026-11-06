//! Starting Caddy.

use super::caddyfile::generate_caddyfile;
use super::db_tool::ensure_database_tool;
use super::logs::{open_log_file_with_retry, LogOutput};
use super::platform::{configure_no_window, terminate_process_by_pid};
use super::platform::{
    find_processes_best_effort, stop_runtime_processes_by_executable, CADDY_STOP_SETTLE,
};
use super::ports::wait_for_port_release;
use super::welcome::write_welcome_page;
use super::{attach_started_process, ServiceProcess};
use crate::constants::{port_check_timeout, process_stop_wait_timeout};
use crate::logs::append_line as append_log_line;
use crate::runtime::locator::RuntimePaths;
use std::process::Command;
use std::thread;

/// Start Caddy web server
pub(super) fn start_caddy(
    service_process: &mut ServiceProcess,
    paths: &RuntimePaths,
    php_port: u16,
    mysql_port: u16,
    postgresql_port: u16,
    selection: &crate::runtime::packages::PackageSelection,
) -> Result<(), String> {
    let database_tool_id = selection.phpmyadmin.as_str();
    // ตรวจสอบและหยุด Caddy processes ที่ซ้ำซ้อนอัตโนมัติ (เหมือน MySQL)
    cleanup_duplicate_caddy_processes(paths, service_process.port);

    if !wait_for_port_release(service_process.port, port_check_timeout()) {
        let stopped = stop_runtime_processes_by_executable(&paths.caddy, "Caddy");
        if stopped > 0 {
            let _ = wait_for_port_release(service_process.port, process_stop_wait_timeout());
        }
    }

    if !wait_for_port_release(service_process.port, port_check_timeout()) {
        return Err(format!(
            "Port {} is still in use. Stop the existing web server on this port and try again.",
            service_process.port
        ));
    }

    // Prepare the selected database tool in the writable config directory. This avoids writing into
    // Program Files or any other install directory that may require elevation.
    ensure_database_tool(
        paths,
        service_process.port,
        mysql_port,
        postgresql_port,
        database_tool_id,
        selection.is_postgresql(),
    )?;

    // Always regenerate Caddyfile with current port settings
    let caddyfile_path = paths.config_dir.join("Caddyfile");
    let welcome_dir = write_welcome_page(paths, database_tool_id)?;
    generate_caddyfile(
        &caddyfile_path,
        paths,
        service_process.port,
        php_port,
        &welcome_dir,
    )?;

    // Open log file with retry logic for Windows file locking
    let log_path = paths.logs_dir.join("caddy.log");
    let log_file = open_log_file_with_retry(&log_path, "Caddy")?;

    // Start Caddy
    let child = configure_no_window(Command::new(&paths.caddy))
        .arg("run")
        .arg("--config")
        .arg(&caddyfile_path)
        .current_dir(&paths.config_dir)
        .log_output_to(log_file, "Caddy")?
        .spawn()
        .map_err(|e| format!("Failed to start Caddy: {}", e))?;

    attach_started_process(service_process, child, log_path, "Caddy")
}

/// Cleanup duplicate / stale Caddy processes before starting.
/// Prevents "port already in use" errors when a previous Caddy instance
/// was left running after CHAMP crashed or was force-quit.
pub(super) fn cleanup_duplicate_caddy_processes(paths: &RuntimePaths, port: u16) {
    let log_path = paths.logs_dir.join("caddy.log");

    let caddy_pids = find_processes_best_effort(&paths.caddy, &log_path);

    if caddy_pids.is_empty() {
        return;
    }

    if caddy_pids.len() > 1 {
        append_log_line(
            &log_path,
            &format!(
                "WARNING: Found {} Caddy processes running. Cleaning up duplicates...",
                caddy_pids.len()
            ),
        );
    } else {
        append_log_line(
            &log_path,
            &format!(
                "Found a stale Caddy process (pid {}). Stopping it before re-launch...",
                caddy_pids[0]
            ),
        );
    }

    let mut stopped = 0;
    for pid in &caddy_pids {
        if terminate_process_by_pid(*pid).is_ok() {
            stopped += 1;
            append_log_line(
                &log_path,
                &format!("Stopped stale Caddy process (pid {})", pid),
            );
        }
    }

    if stopped > 0 {
        append_log_line(
            &log_path,
            &format!("Cleaned up {} stale Caddy process(es)", stopped),
        );
        thread::sleep(CADDY_STOP_SETTLE);
        let _ = wait_for_port_release(port, process_stop_wait_timeout());
    }
}
