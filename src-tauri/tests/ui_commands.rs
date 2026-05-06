#[cfg(test)]
mod ui_command_tests {
    use tauri_app_lib::{ProcessManager, ServiceInfo, ServiceMap, ServiceState, ServiceType};

    /// Test that all service statuses can be retrieved
    #[test]
    fn test_get_all_statuses_returns_all_services() {
        let manager = ProcessManager::new();
        let statuses = manager.get_all_statuses();

        // Should return all three services
        assert_eq!(statuses.len(), 3);
        assert!(statuses.contains_key(&ServiceType::Caddy));
        assert!(statuses.contains_key(&ServiceType::PhpFpm));
        assert!(statuses.contains_key(&ServiceType::MySQL));
    }

    /// Test that initial service state is Stopped
    #[test]
    fn test_initial_service_state_is_stopped() {
        let manager = ProcessManager::new();
        let statuses = manager.get_all_statuses();

        for (service_type, service_info) in statuses.iter() {
            assert_eq!(
                service_info.state,
                ServiceState::Stopped,
                "Service {:?} should be stopped initially",
                service_type
            );
        }
    }

    /// Test that service type has correct display names
    #[test]
    fn test_service_type_display_names() {
        assert_eq!(ServiceType::Caddy.display_name(), "Caddy");
        assert_eq!(ServiceType::PhpFpm.display_name(), "PHP-FPM 8.5");
        assert_eq!(ServiceType::MySQL.display_name(), "MySQL");
    }

    /// Test that service type has correct ports
    #[test]
    fn test_service_type_ports() {
        assert_eq!(ServiceType::Caddy.default_port(), 8080);
        assert_eq!(ServiceType::PhpFpm.default_port(), 9000);
        assert_eq!(ServiceType::MySQL.default_port(), 3306);
    }

    /// Test that service type has correct descriptions
    #[test]
    fn test_service_type_descriptions() {
        assert_eq!(ServiceType::Caddy.description(), "Web Server");
        assert_eq!(ServiceType::PhpFpm.description(), "PHP Runtime");
        assert_eq!(ServiceType::MySQL.description(), "Database Server");
    }

    /// Test service state serialization for frontend
    #[test]
    fn test_service_state_serialization() {
        // Test all states can be serialized to JSON
        let states = vec![
            ServiceState::Stopped,
            ServiceState::Starting,
            ServiceState::Running,
            ServiceState::Stopping,
            ServiceState::Error,
        ];

        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            assert!(!json.is_empty());

            // Test deserialization
            let deserialized: ServiceState = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, state);
        }
    }

    /// Test service type serialization for frontend
    #[test]
    fn test_service_type_serialization() {
        // Test PhpFpm serializes as "php-fpm" for frontend compatibility
        let php_fpm_json = serde_json::to_string(&ServiceType::PhpFpm).unwrap();
        assert_eq!(php_fpm_json, "\"php-fpm\"");

        // Test deserialization
        let deserialized: ServiceType = serde_json::from_str("\"php-fpm\"").unwrap();
        assert_eq!(deserialized, ServiceType::PhpFpm);

        // Test other service types
        let caddy_json = serde_json::to_string(&ServiceType::Caddy).unwrap();
        assert_eq!(caddy_json, "\"caddy\"");

        let mysql_json = serde_json::to_string(&ServiceType::MySQL).unwrap();
        assert_eq!(mysql_json, "\"mysql\"");
    }

    /// Test that status check returns correct state
    #[test]
    fn test_status_check_returns_correct_state() {
        let manager = ProcessManager::new();
        let status = manager.status(ServiceType::Caddy);
        assert_eq!(status, ServiceState::Stopped);
    }

    /// Test that stopping already stopped service is idempotent
    #[test]
    fn test_stop_stopped_service_is_ok() {
        let mut manager = ProcessManager::new();

        // Should not error when stopping already stopped service
        let result = manager.stop(ServiceType::Caddy);
        assert!(result.is_ok());
    }

    /// Test ServiceInfo creation
    #[test]
    fn test_service_info_creation() {
        let info = ServiceInfo::new(ServiceType::Caddy);

        assert_eq!(info.service_type, ServiceType::Caddy);
        assert_eq!(info.state, ServiceState::Stopped);
        assert_eq!(info.port, 8080);
        assert!(info.error_message.is_none());
    }

    /// Test ServiceInfo serialization
    #[test]
    fn test_service_info_serialization() {
        let info = ServiceInfo::new(ServiceType::PhpFpm);

        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.is_empty());

        let deserialized: ServiceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.service_type, ServiceType::PhpFpm);
        assert_eq!(deserialized.port, 9000);
    }

    /// Test that all service types are hashable
    #[test]
    fn test_all_service_types_hashable() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ServiceType::Caddy);
        set.insert(ServiceType::PhpFpm);
        set.insert(ServiceType::MySQL);

        assert_eq!(set.len(), 3);
    }

    /// Test service port assignment
    #[test]
    fn test_service_port_assignment() {
        let manager = ProcessManager::new();
        let statuses = manager.get_all_statuses();

        assert_eq!(statuses[&ServiceType::Caddy].port, 8080);
        assert_eq!(statuses[&ServiceType::PhpFpm].port, 9000);
        assert_eq!(statuses[&ServiceType::MySQL].port, 3306);
    }

    /// Test ServiceMap operations
    #[test]
    fn test_service_map_operations() {
        let mut map: ServiceMap = ServiceMap::new();

        map.insert(ServiceType::Caddy, ServiceInfo::new(ServiceType::Caddy));
        map.insert(ServiceType::PhpFpm, ServiceInfo::new(ServiceType::PhpFpm));
        map.insert(ServiceType::MySQL, ServiceInfo::new(ServiceType::MySQL));

        assert_eq!(map.len(), 3);
        assert!(map.contains_key(&ServiceType::Caddy));
        assert!(map.contains_key(&ServiceType::PhpFpm));
        assert!(map.contains_key(&ServiceType::MySQL));
    }

    /// Test ServiceMap serialization
    #[test]
    fn test_service_map_serialization() {
        let mut map: ServiceMap = ServiceMap::new();
        map.insert(ServiceType::Caddy, ServiceInfo::new(ServiceType::Caddy));
        map.insert(ServiceType::PhpFpm, ServiceInfo::new(ServiceType::PhpFpm));
        map.insert(ServiceType::MySQL, ServiceInfo::new(ServiceType::MySQL));

        // Serialize to JSON
        let json = serde_json::to_string(&map).unwrap();
        assert!(!json.is_empty());

        // Deserialize back
        let deserialized: ServiceMap = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 3);
    }

    /// Test service state is_running method
    #[test]
    fn test_service_state_is_running() {
        assert!(ServiceState::Running.is_running());
        assert!(!ServiceState::Stopped.is_running());
        assert!(!ServiceState::Starting.is_running());
        assert!(!ServiceState::Stopping.is_running());
        assert!(!ServiceState::Error.is_running());
    }

    /// Test service state is_transitioning method
    #[test]
    fn test_service_state_is_transitioning() {
        assert!(ServiceState::Starting.is_transitioning());
        assert!(ServiceState::Stopping.is_transitioning());
        assert!(!ServiceState::Stopped.is_transitioning());
        assert!(!ServiceState::Running.is_transitioning());
        assert!(!ServiceState::Error.is_transitioning());
    }
}
