use crate::config::DEFAULT_PORTS;

pub mod command;
pub mod job;
pub mod manager;
pub mod runner;
pub mod terminal;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    Caddy,
    #[serde(rename = "php-fpm")]
    PhpFpm,
    MySQL,
    #[serde(rename = "postgresql")]
    PostgreSQL,
}

impl ServiceType {
    pub fn default_port(&self) -> u16 {
        match self {
            ServiceType::Caddy => DEFAULT_PORTS.web,
            ServiceType::PhpFpm => DEFAULT_PORTS.php,
            ServiceType::MySQL => DEFAULT_PORTS.mysql,
            ServiceType::PostgreSQL => DEFAULT_PORTS.postgresql,
        }
    }

    /// The stable lowercase id: matches the serde name and the knowledge base.
    pub fn key(&self) -> &'static str {
        match self {
            ServiceType::Caddy => "caddy",
            ServiceType::PhpFpm => "php-fpm",
            ServiceType::MySQL => "mysql",
            ServiceType::PostgreSQL => "postgresql",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ServiceType::Caddy => "Caddy",
            ServiceType::PhpFpm => "PHP-FPM",
            ServiceType::MySQL => "MySQL",
            ServiceType::PostgreSQL => "PostgreSQL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

impl ServiceState {
    pub fn is_running(&self) -> bool {
        matches!(self, ServiceState::Running)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub service_type: ServiceType,
    pub state: ServiceState,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl ServiceInfo {
    pub fn new(service_type: ServiceType) -> Self {
        Self {
            port: service_type.default_port(),
            service_type,
            state: ServiceState::Stopped,
            error_message: None,
        }
    }
}

pub type ServiceMap = std::collections::HashMap<ServiceType, ServiceInfo>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_type_default_ports() {
        assert_eq!(ServiceType::Caddy.default_port(), 8080);
        assert_eq!(ServiceType::PhpFpm.default_port(), 9000);
        assert_eq!(ServiceType::MySQL.default_port(), 3306);
        assert_eq!(ServiceType::PostgreSQL.default_port(), 5432);
    }

    #[test]
    fn test_service_type_display_names() {
        assert_eq!(ServiceType::Caddy.display_name(), "Caddy");
        assert_eq!(ServiceType::PhpFpm.display_name(), "PHP-FPM");
        assert_eq!(ServiceType::MySQL.display_name(), "MySQL");
        assert_eq!(ServiceType::PostgreSQL.display_name(), "PostgreSQL");
    }

    #[test]
    fn test_service_type_serialization() {
        let php_fpm = ServiceType::PhpFpm;
        let serialized = serde_json::to_string(&php_fpm).unwrap();
        assert_eq!(serialized, "\"php-fpm\"");
    }

    #[test]
    fn test_service_type_deserialization() {
        let json = "\"php-fpm\"";
        let deserialized: ServiceType = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, ServiceType::PhpFpm);
    }

    #[test]
    fn test_service_state_is_running() {
        assert!(!ServiceState::Stopped.is_running());
        assert!(!ServiceState::Starting.is_running());
        assert!(ServiceState::Running.is_running());
        assert!(!ServiceState::Stopping.is_running());
        assert!(!ServiceState::Error.is_running());
    }

    #[test]
    fn test_service_info_new() {
        let info = ServiceInfo::new(ServiceType::Caddy);
        assert_eq!(info.service_type, ServiceType::Caddy);
        assert_eq!(info.state, ServiceState::Stopped);
        assert_eq!(info.port, 8080);
        assert!(info.error_message.is_none());
    }

    #[test]
    fn test_service_info_serialization() {
        let info = ServiceInfo {
            service_type: ServiceType::PhpFpm,
            state: ServiceState::Running,
            port: 9000,
            error_message: None,
        };

        let serialized = serde_json::to_string(&info).unwrap();
        let json: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(json["service_type"], "php-fpm");
        assert_eq!(json["state"], "running");
        assert_eq!(json["port"], 9000);
    }

    #[test]
    fn test_service_map_operations() {
        let mut map: ServiceMap = ServiceMap::new();

        let caddy_info = ServiceInfo::new(ServiceType::Caddy);
        map.insert(ServiceType::Caddy, caddy_info);

        let php_info = ServiceInfo::new(ServiceType::PhpFpm);
        map.insert(ServiceType::PhpFpm, php_info);

        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&ServiceType::PhpFpm));
    }

    #[test]
    fn test_all_service_types_hashable() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ServiceType::Caddy);
        set.insert(ServiceType::PhpFpm);
        set.insert(ServiceType::MySQL);
        set.insert(ServiceType::PostgreSQL);

        assert_eq!(set.len(), 4);
    }
}
