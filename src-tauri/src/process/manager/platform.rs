//! OS process primitives and runtime stale-process cleanup helpers.
//!
//! Moved verbatim from the former monolithic `manager.rs` (code-health M-01).

use super::*;

// Windows-specific: Constant to hide console window
#[cfg(target_os = "windows")]
pub(super) const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Configure command to hide console window on Windows
#[cfg(target_os = "windows")]
pub(super) fn configure_no_window(mut command: Command) -> Command {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(target_os = "windows"))]
pub(super) fn configure_no_window(command: Command) -> Command {
    command
}

/// Wait for a child process to exit, polling until `timeout` elapses.
///
/// A kill signal is assumed to have already been sent. This polls `try_wait`
/// in short intervals so the caller is not blocked indefinitely on a process
/// that refuses to exit. If the process is still alive when the deadline is
/// reached, one final `kill` + `try_wait` is attempted before giving up.
pub(super) fn wait_child_with_timeout(child: &mut Child, timeout: std::time::Duration) {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => return,
        }
    }

    // Still alive at the deadline: force another kill and reap once more.
    let _ = child.kill();
    let _ = child.try_wait();
}

pub(super) fn stop_runtime_processes_by_executable(
    executable: &Path,
    service_label: &str,
) -> Result<usize, String> {
    let pids = find_process_ids_by_executable(executable)?;
    let mut stopped = 0;
    for pid in pids {
        if terminate_process_by_pid(pid).is_ok() {
            stopped += 1;
        }
    }
    if stopped > 0 {
        thread::sleep(std::time::Duration::from_millis(250));
    }
    eprintln!(
        "Stopped {} stale {} process(es) for {}",
        stopped,
        service_label,
        executable.display()
    );
    Ok(stopped)
}

#[cfg(target_os = "windows")]
pub(super) fn find_process_ids_by_executable(executable: &Path) -> Result<Vec<u32>, String> {
    // Derive the process name (e.g. "caddy.exe") from the path so this function
    // works for any runtime binary, not just Caddy.
    let proc_name = executable
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("caddy.exe")
        .replace('\'', "''");

    let full_path = executable
        .canonicalize()
        .unwrap_or_else(|_| executable.to_path_buf())
        .to_string_lossy()
        .replace('\'', "''");

    let script = format!(
        "$target = '{}'; \
         Get-CimInstance Win32_Process -Filter \"Name = '{}'\" | \
         Where-Object {{ $_.ExecutablePath -and ([System.IO.Path]::GetFullPath($_.ExecutablePath) -ieq $target) }} | \
         ForEach-Object {{ $_.ProcessId }}",
        full_path, proc_name
    );

    let mut cmd = configure_no_window(Command::new("powershell"));
    let output = cmd
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("Failed to inspect {} processes: {}", proc_name, e))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to inspect {} processes: {}",
            proc_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect())
}

/// Unix implementation: uses `pgrep -f <path>` to find processes whose
/// command line contains the exact executable path, then verifies each
/// candidate by resolving its `/proc/<pid>/exe` symlink (Linux) or via
/// `lsof` (macOS) so we never accidentally kill a system binary with the
/// same name.
#[cfg(not(target_os = "windows"))]
pub(super) fn find_process_ids_by_executable(executable: &Path) -> Result<Vec<u32>, String> {
    // Resolve to a canonical absolute path so the comparison is reliable
    // even if the caller passed a relative path.
    let canonical = executable
        .canonicalize()
        .unwrap_or_else(|_| executable.to_path_buf());
    let canonical_str = canonical.to_string_lossy();

    // `pgrep -f` searches the full command line, giving us candidate PIDs
    // that *mention* our executable path.  We then verify each one below.
    let output = Command::new("pgrep")
        .arg("-f")
        .arg(canonical_str.as_ref())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => {
            // pgrep is not available (rare, but handle gracefully)
            return Ok(Vec::new());
        }
    };

    // pgrep exits with code 1 when no matches found – that is not an error.
    if !output.status.success() && output.stdout.is_empty() {
        return Ok(Vec::new());
    }

    let candidate_pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect();

    // Secondary verification: make sure the resolved executable of each
    // PID is exactly our binary (guards against false positives from
    // processes that just have our path as an argument).
    let verified: Vec<u32> = candidate_pids
        .into_iter()
        .filter(|&pid| {
            // Skip our own process
            if pid == std::process::id() {
                return false;
            }

            #[cfg(target_os = "linux")]
            {
                // On Linux, /proc/<pid>/exe is a symlink to the real binary.
                let proc_exe = format!("/proc/{}/exe", pid);
                match std::fs::read_link(&proc_exe) {
                    Ok(resolved) => resolved == canonical,
                    Err(_) => false,
                }
            }

            #[cfg(target_os = "macos")]
            {
                // On macOS there is no /proc; use `lsof -p <pid>` and look
                // for a "txt" (text/executable) entry that matches our path.
                let lsof_out = Command::new("lsof")
                    .args(["-p", &pid.to_string()])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output();

                match lsof_out {
                    Ok(o) => {
                        let text = String::from_utf8_lossy(&o.stdout);
                        text.lines().any(|line| {
                            let cols: Vec<&str> = line
                                .splitn(10, char::is_whitespace)
                                .filter(|s| !s.is_empty())
                                .collect();
                            // lsof columns: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
                            // FD == "txt" identifies the executable
                            cols.get(3).copied() == Some("txt")
                                && cols
                                    .last()
                                    .map(|name| *name == canonical_str.as_ref())
                                    .unwrap_or(false)
                        })
                    }
                    Err(_) => {
                        // lsof not available: accept the pgrep match as-is
                        true
                    }
                }
            }

            // Fallback for any other Unix-like OS
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            {
                true
            }
        })
        .collect();

    Ok(verified)
}

// R-11: `update_health` polls process liveness on every frontend refresh
// (~3.5s) whenever CHAMP adopted an existing/external process. On Windows each
// check spawns `tasklist`, so an adopted process spawns a process continuously.
// A short time-based cache collapses bursts of repeated checks for the same PID.
pub(super) const PROCESS_EXISTS_TTL: Duration = Duration::from_millis(1500);

#[allow(clippy::type_complexity)]
pub(super) static PROCESS_EXISTS_CACHE: OnceLock<Mutex<HashMap<u32, (Instant, bool)>>> =
    OnceLock::new();

pub(super) fn process_exists(pid: u32) -> bool {
    let cache = PROCESS_EXISTS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    // Return a fresh cached result if one exists and is still within the TTL.
    {
        let guard = cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(&(recorded_at, result)) = guard.get(&pid) {
            if recorded_at.elapsed() < PROCESS_EXISTS_TTL {
                return result;
            }
        }
    }

    let result = process_exists_uncached(pid);

    {
        let mut guard = cache.lock().unwrap_or_else(PoisonError::into_inner);
        guard.insert(pid, (Instant::now(), result));
    }

    result
}

#[cfg(target_os = "windows")]
pub(super) fn process_exists_uncached(pid: u32) -> bool {
    let mut cmd = configure_no_window(Command::new("tasklist"));
    cmd.args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&format!("\"{}\"", pid))
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
pub(super) fn process_exists_uncached(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
pub(super) fn terminate_process_by_pid(pid: u32) -> Result<(), String> {
    let mut cmd = configure_no_window(Command::new("taskkill"));
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to stop process pid {}: {}", pid, e))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("taskkill failed for process pid {}", pid))
            }
        })
}

#[cfg(not(target_os = "windows"))]
pub(super) fn terminate_process_by_pid(pid: u32) -> Result<(), String> {
    Command::new("kill")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to stop process pid {}: {}", pid, e))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("kill failed for process pid {}", pid))
            }
        })
}

// ---------------------------------------------------------------------------
// Caddy stale-process helpers (mirrors the MySQL pattern)
// ---------------------------------------------------------------------------

/// Find all Caddy processes currently running from our runtime executable
pub(super) fn find_all_caddy_processes(caddy_executable: &Path) -> Result<Vec<u32>, String> {
    find_process_ids_by_executable(caddy_executable)
}

/// Cleanup duplicate / stale Caddy processes before starting.
/// Prevents "port already in use" errors when a previous Caddy instance
/// was left running after CHAMP crashed or was force-quit.
pub(super) fn cleanup_duplicate_caddy_processes(
    paths: &RuntimePaths,
    port: u16,
) -> Result<(), String> {
    let log_path = paths.logs_dir.join("caddy.log");

    let caddy_pids = find_all_caddy_processes(&paths.caddy)?;

    if caddy_pids.is_empty() {
        return Ok(());
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
        thread::sleep(std::time::Duration::from_millis(500));
        let _ = wait_for_port_release(port, std::time::Duration::from_secs(5));
    }

    Ok(())
}

/// Force stop all Caddy processes (used when stopping the service).
/// Ensures no Caddy processes are left running even if the child handle
/// was already lost (e.g. after a crash).
pub(super) fn force_stop_all_caddy_processes(caddy_executable: &Path) -> Result<usize, String> {
    let pids = find_all_caddy_processes(caddy_executable)?;

    if pids.is_empty() {
        return Ok(0);
    }

    eprintln!("Force stopping {} Caddy process(es)...", pids.len());

    let mut stopped = 0;
    for pid in pids {
        if terminate_process_by_pid(pid).is_ok() {
            stopped += 1;
            eprintln!("Stopped Caddy process (pid {})", pid);
        }
    }

    if stopped > 0 {
        thread::sleep(std::time::Duration::from_millis(500));
    }

    Ok(stopped)
}

// ---------------------------------------------------------------------------

/// Cleanup duplicate MySQL processes automatically
/// This prevents the "exit code 1" error caused by multiple MySQL instances
pub(super) fn cleanup_duplicate_mysql_processes(
    paths: &RuntimePaths,
    port: u16,
) -> Result<(), String> {
    let log_path = paths.logs_dir.join("mysql.log");

    // ค้นหา MySQL processes ที่เป็นของ CHAMP เท่านั้น (ตรวจ full path ไม่ใช่แค่ชื่อไฟล์)
    // เพื่อไม่ให้ไปฆ่า MySQL/MariaDB ที่ผู้ใช้ติดตั้งเองบนเครื่อง (R-03)
    let mysql_pids = find_process_ids_by_executable(&paths.mysql)?;

    if mysql_pids.is_empty() {
        return Ok(());
    }

    // ถ้ามีมากกว่า 1 process แสดงว่ามีซ้ำซ้อน
    if mysql_pids.len() > 1 {
        append_log_line(
            &log_path,
            &format!(
                "WARNING: Found {} MySQL processes running. Cleaning up duplicates...",
                mysql_pids.len()
            ),
        );

        // หยุดทุก process
        let mut stopped = 0;
        for pid in &mysql_pids {
            if terminate_process_by_pid(*pid).is_ok() {
                stopped += 1;
                append_log_line(
                    &log_path,
                    &format!("Stopped duplicate MySQL process (pid {})", pid),
                );
            }
        }

        if stopped > 0 {
            append_log_line(
                &log_path,
                &format!("Cleaned up {} duplicate MySQL process(es)", stopped),
            );

            // รอให้ processes หยุดและ port ว่าง
            thread::sleep(std::time::Duration::from_secs(2));
            let _ = wait_for_port_release(port, std::time::Duration::from_secs(5));
        }
    } else if mysql_pids.len() == 1 {
        // มี 1 process แต่อาจจะไม่ใช่ของเรา หรือ port ไม่ตรง
        let pid = mysql_pids[0];

        // ตรวจสอบว่า process นี้ใช้ port ที่เราต้องการหรือไม่
        if !tcp_port_accepts(port) {
            // Process มีอยู่แต่ไม่ได้ใช้ port ของเรา - อาจจะ crashed หรือ starting
            if !process_exists(pid) {
                append_log_line(
                    &log_path,
                    &format!("Found stale MySQL PID {} (process not running)", pid),
                );
            }
        }
    }

    Ok(())
}

/// Force stop all MySQL processes (used when stopping service)
/// This ensures no MySQL processes are left running
pub(super) fn force_stop_all_mysql_processes(mysql_executable: &Path) -> Result<usize, String> {
    // Only match processes that resolve to *our* mysqld binary (full-path
    // verified), so a user's own MySQL/MariaDB install is never force-killed. (R-03)
    let pids = find_process_ids_by_executable(mysql_executable)?;

    if pids.is_empty() {
        return Ok(0);
    }

    eprintln!("Force stopping {} MySQL process(es)...", pids.len());

    let mut stopped = 0;
    for pid in pids {
        if terminate_process_by_pid(pid).is_ok() {
            stopped += 1;
            eprintln!("Stopped MySQL process (pid {})", pid);
        }
    }

    // รอให้ processes หยุดจริงๆ
    if stopped > 0 {
        thread::sleep(std::time::Duration::from_secs(2));
    }

    Ok(stopped)
}
