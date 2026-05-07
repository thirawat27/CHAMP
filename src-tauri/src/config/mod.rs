pub mod ports;
pub mod settings;

pub use ports::is_port_available;
pub use settings::{AppSettings, DEFAULT_PORTS};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ports::find_available_port;
    use crate::config::settings::DEFAULT_PORTS;

    #[test]
    fn test_default_ports() {
        assert_eq!(DEFAULT_PORTS.web, 8080);
        assert_eq!(DEFAULT_PORTS.php, 9000);
        assert_eq!(DEFAULT_PORTS.mysql, 3306);
    }

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.web_port, 8080);
        assert_eq!(settings.php_port, 9000);
        assert_eq!(settings.mysql_port, 3306);
    }

    #[test]
    fn test_is_port_available() {
        // Port 1 is typically unavailable (reserved)
        // This test just verifies the function works
        let _result = is_port_available(8080);
    }

    #[test]
    fn test_find_available_port() {
        // Should return the preferred port if available
        let port = find_available_port(8080);
        assert!(port >= 8080);
    }
}
