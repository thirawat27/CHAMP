//! Custom error types for CHAMP application
//!
//! This module provides a unified error type that can be used throughout the application,
//! making error handling more consistent and informative.

use std::fmt;

/// Main error type for CHAMP application
#[derive(Debug)]
pub enum ChampError {
    /// Service not found in the process manager
    ServiceNotFound(String),
    
    /// Port is already in use by another process
    PortInUse(u16),
    
    /// Process failed to start or crashed
    ProcessFailed {
        service: String,
        reason: String,
        log_path: Option<String>,
    },
    
    /// Configuration error (invalid settings, missing files, etc.)
    ConfigError(String),
    
    /// IO error (file operations, network, etc.)
    IoError(std::io::Error),
    
    /// Runtime binary not found or invalid
    RuntimeError(String),
    
    /// Database initialization or operation failed
    DatabaseError(String),
    
    /// Download or extraction failed
    DownloadError(String),
    
    /// Invalid port number
    InvalidPort(u16),
    
    /// Service is in wrong state for the requested operation
    InvalidState {
        service: String,
        current_state: String,
        expected_state: String,
    },
}

impl fmt::Display for ChampError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChampError::ServiceNotFound(name) => {
                write!(f, "Service not found: {}", name)
            }
            ChampError::PortInUse(port) => {
                write!(
                    f,
                    "Port {} is already in use. Please stop the conflicting process or change the port in Settings.",
                    port
                )
            }
            ChampError::ProcessFailed { service, reason, log_path } => {
                write!(f, "{} process failed: {}", service, reason)?;
                if let Some(path) = log_path {
                    write!(f, "\nLog file: {}", path)?;
                }
                Ok(())
            }
            ChampError::ConfigError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            ChampError::IoError(err) => {
                write!(f, "IO error: {}", err)
            }
            ChampError::RuntimeError(msg) => {
                write!(f, "Runtime error: {}", msg)
            }
            ChampError::DatabaseError(msg) => {
                write!(f, "Database error: {}", msg)
            }
            ChampError::DownloadError(msg) => {
                write!(f, "Download error: {}", msg)
            }
            ChampError::InvalidPort(port) => {
                write!(f, "Invalid port number: {}. Port must be between 1 and 65535.", port)
            }
            ChampError::InvalidState { service, current_state, expected_state } => {
                write!(
                    f,
                    "Service {} is in state '{}' but expected '{}' for this operation",
                    service, current_state, expected_state
                )
            }
        }
    }
}

impl std::error::Error for ChampError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ChampError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

// Conversions from other error types
impl From<std::io::Error> for ChampError {
    fn from(err: std::io::Error) -> Self {
        ChampError::IoError(err)
    }
}

impl From<serde_json::Error> for ChampError {
    fn from(err: serde_json::Error) -> Self {
        ChampError::ConfigError(format!("JSON error: {}", err))
    }
}

impl From<tauri::Error> for ChampError {
    fn from(err: tauri::Error) -> Self {
        ChampError::RuntimeError(format!("Tauri error: {}", err))
    }
}

// Convert ChampError to String for Tauri commands
impl From<ChampError> for String {
    fn from(err: ChampError) -> Self {
        err.to_string()
    }
}

/// Result type alias for CHAMP operations
pub type Result<T> = std::result::Result<T, ChampError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ChampError::ServiceNotFound("test-service".to_string());
        assert_eq!(err.to_string(), "Service not found: test-service");

        let err = ChampError::PortInUse(8080);
        assert!(err.to_string().contains("8080"));
        assert!(err.to_string().contains("already in use"));
    }

    #[test]
    fn test_process_failed_with_log() {
        let err = ChampError::ProcessFailed {
            service: "MySQL".to_string(),
            reason: "Port conflict".to_string(),
            log_path: Some("/path/to/mysql.log".to_string()),
        };
        let msg = err.to_string();
        assert!(msg.contains("MySQL"));
        assert!(msg.contains("Port conflict"));
        assert!(msg.contains("/path/to/mysql.log"));
    }

    #[test]
    fn test_invalid_state() {
        let err = ChampError::InvalidState {
            service: "Caddy".to_string(),
            current_state: "stopped".to_string(),
            expected_state: "running".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Caddy"));
        assert!(msg.contains("stopped"));
        assert!(msg.contains("running"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let champ_err: ChampError = io_err.into();
        assert!(matches!(champ_err, ChampError::IoError(_)));
    }

    #[test]
    fn test_string_conversion() {
        let err = ChampError::ConfigError("test error".to_string());
        let s: String = err.into();
        assert_eq!(s, "Configuration error: test error");
    }
}
