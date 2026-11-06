//! Helpers shared by the MySQL and PostgreSQL start paths.

use super::platform::{process_exists_uncached, terminate_process_by_pid};
use super::ports::tcp_port_accepts;
use crate::constants::{
    database_init_poll, database_init_timeout, process_probe_delay, process_stop_wait_timeout,
    EXISTING_DATABASE_PROBE_ATTEMPTS,
};
use crate::logs::append_line as append_log_line;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

/// Give a process that may just be coming up a few seconds to serve its port.
pub(super) fn wait_port_accepting(port: u16) -> bool {
    for _ in 0..EXISTING_DATABASE_PROBE_ATTEMPTS {
        if tcp_port_accepts(port) {
            return true;
        }
        thread::sleep(process_probe_delay());
    }
    false
}

pub(super) fn stop_process_holding_data_dir(
    pid: u32,
    label: &str,
    log_path: &Path,
) -> Result<(), String> {
    append_log_line(
        log_path,
        &format!(
            "CHAMP {} process (pid {}) still holds the data directory on another port. Stopping it before start...",
            label, pid
        ),
    );
    terminate_process_by_pid(pid)?;
    let deadline = Instant::now() + process_stop_wait_timeout();
    while process_exists_uncached(pid) {
        if Instant::now() >= deadline {
            return Err(format!(
                "{} process (pid {}) is still running and locking the data directory. Stop it and try again.",
                label, pid
            ));
        }
        thread::sleep(process_probe_delay());
    }
    Ok(())
}

/// Where a database initializes before moving to its real data directory.
///
/// Refuses a target that already has files: they are not a finished database
/// (the caller checked that) but they may be someone's data, so never replace them.
pub(super) fn prepare_init_staging_dir(target: &Path, label: &str) -> Result<PathBuf, String> {
    let has_files = fs::read_dir(target).is_ok_and(|mut entries| entries.next().is_some());
    if has_files {
        return Err(format!(
            "{} data directory {} has files but is not a complete database. \
             An earlier initialization may have been interrupted. Move or delete the folder, then start again.",
            label,
            target.display()
        ));
    }

    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "data".to_string());
    let staging = target.with_file_name(format!("{name}.initializing"));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| {
            format!(
                "Failed to clear interrupted initialization at {}: {}",
                staging.display(),
                e
            )
        })?;
    }
    if let Some(parent) = staging.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create database folder {}: {}",
                parent.display(),
                e
            )
        })?;
    }
    Ok(staging)
}

/// Antivirus scanners often hold a freshly written tree open for a moment.
const INIT_PROMOTE_ATTEMPTS: usize = 20;

const INIT_PROMOTE_RETRY_DELAY: Duration = Duration::from_millis(250);

pub(super) fn promote_init_staging_dir(staging: &Path, target: &Path) -> Result<(), String> {
    // `initialize` created an empty target; remove_dir refuses anything else.
    let _ = fs::remove_dir(target);
    let mut attempt = 1;
    loop {
        match fs::rename(staging, target) {
            Ok(()) => return Ok(()),
            Err(_) if attempt < INIT_PROMOTE_ATTEMPTS => {
                attempt += 1;
                thread::sleep(INIT_PROMOTE_RETRY_DELAY);
            }
            Err(error) => {
                return Err(format!(
                    "Failed to move initialized database from {} to {}: {}",
                    staging.display(),
                    target.display(),
                    error
                ));
            }
        }
    }
}

/// Wait for a first-time database initialization, killing it past the timeout.
pub(super) fn wait_for_database_init(
    child: &mut Child,
    log_path: &Path,
    label: &str,
) -> Result<(), String> {
    let start = Instant::now();
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) => {
                if start.elapsed() > database_init_timeout() {
                    eprintln!("{label} initialization timeout, killing process");
                    let _ = child.kill();
                    let _ = child.wait();
                    break false;
                }
                thread::sleep(database_init_poll());
            }
            Err(_) => break false,
        }
    };
    if success {
        return Ok(());
    }

    let output = fs::read_to_string(log_path).unwrap_or_default();
    eprintln!("{label} initialization failed. Output:\n{output}");
    Err(format!(
        "{label} initialization failed. Check the log file at: {log_path:?}"
    ))
}
