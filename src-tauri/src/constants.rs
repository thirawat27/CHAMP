//! Application-wide constants
//!
//! This module contains all magic numbers and hardcoded values used throughout the application.
//! Centralizing these values makes them easier to maintain and modify.

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

/// Timeout for MySQL port release after stop (seconds)
pub const MYSQL_PORT_RELEASE_TIMEOUT_SECS: u64 = 8;

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

// ============================================================================
// System Metrics
// ============================================================================

/// Minimum interval between system metrics samples (milliseconds)
/// This constant is used in commands.rs for system metrics monitoring
#[allow(dead_code)]
pub const SYSTEM_METRICS_MIN_SAMPLE_INTERVAL_MS: u64 = 1500;

// ============================================================================
// Versions
// ============================================================================

/// Default Caddy version (fallback if not in runtime-config.json)
pub const DEFAULT_CADDY_VERSION: &str = "2.11.2";

// ============================================================================
// Helper Functions
// ============================================================================

/// Get MySQL port release timeout as Duration
pub const fn mysql_port_release_timeout() -> Duration {
    Duration::from_secs(MYSQL_PORT_RELEASE_TIMEOUT_SECS)
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

    #[test]
    fn test_constants_are_reasonable() {
        assert!(MAX_LOG_FILE_RETRY > 0);
        assert!(LOG_FILE_RETRY_DELAY_MS > 0);
        assert!(MAX_LOG_TAIL_LINES > 0);
        assert!(MYSQL_PORT_RELEASE_TIMEOUT_SECS > DEFAULT_PORT_RELEASE_TIMEOUT_SECS);
        assert!(PORT_CHECK_TIMEOUT_MS < 1000);
    }

    #[test]
    fn test_duration_helpers() {
        assert_eq!(mysql_port_release_timeout().as_secs(), MYSQL_PORT_RELEASE_TIMEOUT_SECS);
        assert_eq!(default_port_release_timeout().as_secs(), DEFAULT_PORT_RELEASE_TIMEOUT_SECS);
        assert_eq!(port_check_timeout().as_millis(), PORT_CHECK_TIMEOUT_MS as u128);
    }
}
