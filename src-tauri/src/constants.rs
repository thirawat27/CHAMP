//! Application-wide constants
//!
//! Timeouts, retry budgets, and size limits shared across modules. Values local
//! to one module stay next to their code.

use std::time::Duration;

// ============================================================================
// File Operations
// ============================================================================

/// Maximum number of retries when opening log files (Windows file locking)
pub const MAX_LOG_FILE_RETRY: usize = 5;

/// Delay between log file open retries (milliseconds)
pub const LOG_FILE_RETRY_DELAY_MS: u64 = 100;

/// Maximum number of lines to read from log tail for error messages
pub const MAX_LOG_TAIL_LINES: usize = 40;

// ============================================================================
// Process Management
// ============================================================================

/// Timeout for a database's port release after stop (seconds)
pub const DATABASE_PORT_RELEASE_TIMEOUT_SECS: u64 = 8;

/// Timeout for other services port release after stop (seconds)
pub const DEFAULT_PORT_RELEASE_TIMEOUT_SECS: u64 = 4;

/// Short timeout for initial port availability check (milliseconds)
pub const PORT_CHECK_TIMEOUT_MS: u64 = 300;

/// Timeout after stopping conflicting processes (seconds)
pub const PROCESS_STOP_WAIT_TIMEOUT_SECS: u64 = 5;

/// Maximum retries for MySQL initialization check
pub const MYSQL_INIT_MAX_RETRIES: usize = 20;

/// Delay between MySQL initialization checks (milliseconds)
pub const MYSQL_INIT_CHECK_DELAY_MS: u64 = 500;

/// How long a first-time database initialization may run (seconds)
pub const DATABASE_INIT_TIMEOUT_SECS: u64 = 120;

/// Delay between checks on a running database initialization (milliseconds)
pub const DATABASE_INIT_POLL_MS: u64 = 500;

/// Probes for a database already serving its port before it counts as not running
pub const EXISTING_DATABASE_PROBE_ATTEMPTS: usize = 20;

/// Delay between probes of a process's port or exit (milliseconds)
pub const PROCESS_PROBE_DELAY_MS: u64 = 250;

/// Delay between `try_wait` calls on an exiting child (milliseconds)
pub const CHILD_EXIT_POLL_MS: u64 = 50;

/// Delay between attempts to bind a port being released (milliseconds)
pub const PORT_RELEASE_POLL_MS: u64 = 150;

/// How long a database gets to shut down cleanly before it is killed (seconds)
pub const DATABASE_SHUTDOWN_TIMEOUT_SECS: u64 = 10;

/// How long a service gets to act on SIGTERM before it is killed (seconds)
pub const GRACEFUL_STOP_TIMEOUT_SECS: u64 = 3;

/// How long `start` waits for a web or PHP service to accept connections (seconds)
pub const SERVICE_READY_TIMEOUT_SECS: u64 = 10;

/// How long `start` waits for a database to accept connections (seconds).
/// Crash recovery can take a while; the service stays up if this elapses.
pub const DATABASE_READY_TIMEOUT_SECS: u64 = 30;

/// Delay between readiness probes (milliseconds)
pub const READY_POLL_INTERVAL_MS: u64 = 100;

/// `php-errors.log` is rotated on start once it grows past this size (bytes)
pub const PHP_ERROR_LOG_MAX_BYTES: u64 = 50 * 1024 * 1024;

// ============================================================================
// System Metrics
// ============================================================================

/// Minimum interval between system metrics samples (milliseconds)
/// This constant is used in commands.rs for system metrics monitoring
pub const SYSTEM_METRICS_MIN_SAMPLE_INTERVAL_MS: u64 = 1500;

// ============================================================================
// Helper Functions
// ============================================================================

/// Get database port release timeout as Duration
pub const fn database_port_release_timeout() -> Duration {
    Duration::from_secs(DATABASE_PORT_RELEASE_TIMEOUT_SECS)
}

/// Get default port release timeout as Duration
pub const fn default_port_release_timeout() -> Duration {
    Duration::from_secs(DEFAULT_PORT_RELEASE_TIMEOUT_SECS)
}

/// Get port check timeout as Duration
pub const fn port_check_timeout() -> Duration {
    Duration::from_millis(PORT_CHECK_TIMEOUT_MS)
}

/// Get process stop wait timeout as Duration
pub const fn process_stop_wait_timeout() -> Duration {
    Duration::from_secs(PROCESS_STOP_WAIT_TIMEOUT_SECS)
}

/// Get database shutdown timeout as Duration
pub const fn database_shutdown_timeout() -> Duration {
    Duration::from_secs(DATABASE_SHUTDOWN_TIMEOUT_SECS)
}

/// Get SIGTERM grace period as Duration
pub const fn graceful_stop_timeout() -> Duration {
    Duration::from_secs(GRACEFUL_STOP_TIMEOUT_SECS)
}

/// Get web/PHP readiness timeout as Duration
pub const fn service_ready_timeout() -> Duration {
    Duration::from_secs(SERVICE_READY_TIMEOUT_SECS)
}

/// Get database readiness timeout as Duration
pub const fn database_ready_timeout() -> Duration {
    Duration::from_secs(DATABASE_READY_TIMEOUT_SECS)
}

/// Get readiness probe delay as Duration
pub const fn ready_poll_interval() -> Duration {
    Duration::from_millis(READY_POLL_INTERVAL_MS)
}

/// Get database initialization timeout as Duration
pub const fn database_init_timeout() -> Duration {
    Duration::from_secs(DATABASE_INIT_TIMEOUT_SECS)
}

/// Get database initialization poll delay as Duration
pub const fn database_init_poll() -> Duration {
    Duration::from_millis(DATABASE_INIT_POLL_MS)
}

/// Get process probe delay as Duration
pub const fn process_probe_delay() -> Duration {
    Duration::from_millis(PROCESS_PROBE_DELAY_MS)
}

/// Get child exit poll delay as Duration
pub const fn child_exit_poll() -> Duration {
    Duration::from_millis(CHILD_EXIT_POLL_MS)
}

/// Get port release poll delay as Duration
pub const fn port_release_poll() -> Duration {
    Duration::from_millis(PORT_RELEASE_POLL_MS)
}

/// Get MySQL init check delay as Duration
pub const fn mysql_init_check_delay() -> Duration {
    Duration::from_millis(MYSQL_INIT_CHECK_DELAY_MS)
}

/// Get log file retry delay as Duration
pub const fn log_file_retry_delay() -> Duration {
    Duration::from_millis(LOG_FILE_RETRY_DELAY_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const _: () = {
        assert!(MAX_LOG_FILE_RETRY > 0);
        assert!(LOG_FILE_RETRY_DELAY_MS > 0);
        assert!(MAX_LOG_TAIL_LINES > 0);
        assert!(DATABASE_PORT_RELEASE_TIMEOUT_SECS > DEFAULT_PORT_RELEASE_TIMEOUT_SECS);
        assert!(PORT_CHECK_TIMEOUT_MS < 1000);
    };

    #[test]
    fn test_duration_helpers() {
        assert_eq!(
            database_port_release_timeout().as_secs(),
            DATABASE_PORT_RELEASE_TIMEOUT_SECS
        );
        assert_eq!(
            default_port_release_timeout().as_secs(),
            DEFAULT_PORT_RELEASE_TIMEOUT_SECS
        );
        assert_eq!(
            port_check_timeout().as_millis(),
            PORT_CHECK_TIMEOUT_MS as u128
        );
    }
}
