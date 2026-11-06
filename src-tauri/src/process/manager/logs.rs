//! Service log files and process-exit messages.

use crate::constants::{log_file_retry_delay, MAX_LOG_FILE_RETRY, MAX_LOG_TAIL_LINES};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

/// Send a command's stdout and stderr to the same log file.
pub(super) trait LogOutput {
    fn log_output_to(&mut self, log_file: File, label: &str) -> Result<&mut Self, String>;
}

impl LogOutput for Command {
    fn log_output_to(&mut self, log_file: File, label: &str) -> Result<&mut Self, String> {
        let stdout = log_file
            .try_clone()
            .map_err(|e| format!("Failed to duplicate {} log handle: {}", label, e))?;
        Ok(self
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(log_file)))
    }
}

/// Open a log file with retry logic for Windows file locking
pub(super) fn open_log_file_with_retry(
    log_path: &PathBuf,
    service_name: &str,
) -> Result<File, String> {
    for attempt in 0..MAX_LOG_FILE_RETRY {
        // Try to open the file, truncating if it exists (for fresh logs)
        // On subsequent retries, try to append in case another process has it open
        let result = if attempt == 0 {
            File::create(log_path)
        } else {
            OpenOptions::new().create(true).append(true).open(log_path)
        };

        match result {
            Ok(file) => return Ok(file),
            Err(e) => {
                if e.raw_os_error() == Some(32) && attempt < MAX_LOG_FILE_RETRY - 1 {
                    // Windows error 32: file is being used by another process
                    // Wait and retry
                    std::thread::sleep(log_file_retry_delay());
                } else {
                    return Err(format!(
                        "Failed to create {} log file after {} attempts: {}",
                        service_name,
                        attempt + 1,
                        e
                    ));
                }
            }
        }
    }

    Err(format!(
        "Failed to create {} log file: maximum retries exceeded",
        service_name
    ))
}

pub(super) fn format_exit_status(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit code {}", code);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("signal {}", signal);
        }
    }

    format!("{:?}", status)
}

pub(super) fn read_log_tail(log_path: &Path, max_lines: usize) -> Option<String> {
    let lines = crate::logs::read_tail_lines(log_path)?;
    let first = lines.len().saturating_sub(max_lines);
    let tail = lines[first..].join("\n").trim().to_string();
    if tail.is_empty() {
        None
    } else {
        Some(tail)
    }
}

pub(super) fn format_process_exit_error(
    summary: &str,
    status: ExitStatus,
    log_path: Option<&Path>,
) -> String {
    let mut message = format!("{} ({})", summary, format_exit_status(status));
    if let Some(path) = log_path {
        message.push_str(&format!("\nLog file: {}", path.display()));
        if let Some(tail) = read_log_tail(path, MAX_LOG_TAIL_LINES) {
            message.push_str("\nLast log lines:\n");
            message.push_str(&tail);
        }
    }
    message
}
