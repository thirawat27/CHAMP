use super::caddyfile::generate_caddyfile;
use super::database::{prepare_init_staging_dir, promote_init_staging_dir};
use super::htaccess::discover_htaccess_rules;
use super::logs::{format_exit_status, format_process_exit_error, read_log_tail};
use super::mysql::{mysql_server_args, remove_stale_mysql_init_files};
use super::php_config::{apply_php_database_env, rotate_php_error_log};
use super::platform::{find_process_ids_by_executable, lsof_lists_executable, mysql_socket_path};
use super::ports::{find_available_port_excluding, first_fallback_port};
use super::{
    port_dependents, stack_start_services, wait_until_ready, ProcessManager, ServiceProcess,
};
use crate::process::{ServiceState, ServiceType};
use crate::runtime::locator::RuntimePaths;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Stdio;
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

#[cfg(windows)]
fn nonzero_exit_status() -> ExitStatus {
    Command::new("cmd")
        .args(["/C", "exit 7"])
        .status()
        .expect("failed to create nonzero exit status")
}

#[cfg(not(windows))]
fn nonzero_exit_status() -> ExitStatus {
    Command::new("sh")
        .args(["-c", "exit 7"])
        .status()
        .expect("failed to create nonzero exit status")
}

// R-03: MySQL cleanup must match processes by *full path*, not by executable
// name. A binary named `mysqld[.exe]` living under a CHAMP runtime path that
// has nothing running from it must yield zero PIDs, even if the host has its
// own MySQL/MariaDB of the same name running elsewhere. This guards against
// force-killing a user's own database. The name-only `find_all_mysql_processes`
// helper was removed; MySQL now shares Caddy/PostgreSQL's path-verified finder.
#[test]
fn test_find_process_ids_by_executable_ignores_same_name_other_path() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let exe_name = if cfg!(windows) {
        "mysqld.exe"
    } else {
        "mysqld"
    };
    // A path under a throwaway dir: nothing is ever launched from here, so
    // regardless of any real mysqld running on the machine, the result is empty.
    let fake_mysql = temp.path().join("runtime").join("mysql").join(exe_name);

    let pids = find_process_ids_by_executable(&fake_mysql)
        .expect("finder should succeed even when no matching process exists");

    assert!(
        pids.is_empty(),
        "expected no PIDs for a same-named binary at an unused path, got {:?}",
        pids
    );
}

#[test]
fn test_format_exit_status_human_readable() {
    let status = nonzero_exit_status();
    let formatted = format_exit_status(status);
    assert!(formatted.contains("7"), "unexpected format: {}", formatted);
    assert!(
        !formatted.contains("ExitStatus("),
        "status should be user-friendly: {}",
        formatted
    );
}

#[test]
fn test_format_process_exit_error_includes_log_tail() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH")
        .as_nanos();
    let log_path = std::env::temp_dir().join(format!("champ-process-error-{}.log", unique));
    std::fs::write(
        &log_path,
        "line 1\nline 2\nline 3\nfatal startup error on line 4\n",
    )
    .expect("failed to write temp log");

    let status = nonzero_exit_status();
    let message = format_process_exit_error(
        "Service exited unexpectedly",
        status,
        Some(log_path.as_path()),
    );

    assert!(message.contains("exit code 7"));
    assert!(message.contains("Log file:"));
    assert!(message.contains("fatal startup error on line 4"));

    let _ = std::fs::remove_file(log_path);
}

/// Independent of the developer's real settings.json.
fn test_manager() -> ProcessManager {
    ProcessManager::with_settings(crate::config::AppSettings::default())
}

#[test]
fn test_process_manager_new() {
    let manager = test_manager();

    assert_eq!(manager.services.len(), 4);

    let caddy = manager.services.get(&ServiceType::Caddy).unwrap();
    assert_eq!(caddy.name, ServiceType::Caddy);
    assert_eq!(caddy.state, ServiceState::Stopped);
    assert_eq!(caddy.port, 8080);
    assert!(caddy.child.is_none());

    let postgresql = manager.services.get(&ServiceType::PostgreSQL).unwrap();
    assert_eq!(postgresql.name, ServiceType::PostgreSQL);
    assert_eq!(postgresql.state, ServiceState::Stopped);
    assert_eq!(postgresql.port, 5432);
}

#[test]
fn test_process_manager_default() {
    let manager = ProcessManager::default();
    assert_eq!(manager.services.len(), 4);
    assert!(manager.runtime_paths.is_none());
}

#[test]
fn test_status_of_service() {
    let manager = test_manager();

    assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Stopped);
    assert_eq!(manager.status(ServiceType::PhpFpm), ServiceState::Stopped);
    assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Stopped);
    assert_eq!(
        manager.status(ServiceType::PostgreSQL),
        ServiceState::Stopped
    );
}

#[test]
fn test_get_all_statuses() {
    let manager = test_manager();
    let statuses = manager.get_all_statuses();

    assert_eq!(statuses.len(), 4);

    let caddy_info = statuses.get(&ServiceType::Caddy).unwrap();
    assert_eq!(caddy_info.service_type, ServiceType::Caddy);
    assert_eq!(caddy_info.state, ServiceState::Stopped);
    assert_eq!(caddy_info.port, 8080);
}

#[test]
fn test_stop_already_stopped_service() {
    let mut manager = test_manager();

    let result = manager.stop(ServiceType::Caddy);
    assert!(result.is_ok());
    assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Stopped);
}

#[test]
fn test_service_error_state_handling() {
    let mut manager = test_manager();

    let service = manager.services.get_mut(&ServiceType::MySQL).unwrap();
    service.state = ServiceState::Error;
    service.error_message = Some("Test error".to_string());

    assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Error);

    let statuses = manager.get_all_statuses();
    let mysql_info = statuses.get(&ServiceType::MySQL).unwrap();
    assert_eq!(mysql_info.state, ServiceState::Error);
    assert_eq!(mysql_info.error_message, Some("Test error".to_string()));
}

#[test]
fn test_update_health_with_no_processes() {
    let mut manager = test_manager();

    manager.update_health();

    assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Stopped);
    assert_eq!(manager.status(ServiceType::PhpFpm), ServiceState::Stopped);
    assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Stopped);
    assert_eq!(
        manager.status(ServiceType::PostgreSQL),
        ServiceState::Stopped
    );
}

#[test]
fn test_port_assignment_for_services() {
    let manager = test_manager();

    let caddy = manager.services.get(&ServiceType::Caddy).unwrap();
    assert_eq!(caddy.port, 8080);

    let php = manager.services.get(&ServiceType::PhpFpm).unwrap();
    assert_eq!(php.port, 9000);

    let mysql = manager.services.get(&ServiceType::MySQL).unwrap();
    assert_eq!(mysql.port, 3306);

    let postgresql = manager.services.get(&ServiceType::PostgreSQL).unwrap();
    assert_eq!(postgresql.port, 5432);
}

#[test]
fn test_find_available_port_skips_busy_preferred_port() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind temporary port");
    let preferred_port = listener
        .local_addr()
        .expect("failed to read temporary port")
        .port();

    let selected = find_available_port_excluding(ServiceType::Caddy, preferred_port, &[])
        .expect("expected fallback port");

    assert_ne!(selected, preferred_port);
}

#[test]
fn test_find_available_port_skips_reserved_stack_ports() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind temporary port");
    let preferred_port = listener
        .local_addr()
        .expect("failed to read temporary port")
        .port();
    drop(listener);

    let selected =
        find_available_port_excluding(ServiceType::PhpFpm, preferred_port, &[preferred_port])
            .expect("expected fallback port");

    assert_ne!(selected, preferred_port);
}

#[test]
fn test_mysql_default_fallback_does_not_use_first_adjacent_port() {
    assert_eq!(
        first_fallback_port(ServiceType::MySQL, crate::config::DEFAULT_PORTS.mysql),
        crate::config::DEFAULT_PORTS.mysql + 2
    );
}

#[test]
fn test_htaccess_parser_maps_extensionless_php_and_error_document() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let project = temp.path().join("IT-NVC");
    std::fs::create_dir_all(&project).expect("failed to create project dir");
    std::fs::write(
        project.join(".htaccess"),
        r#"
RewriteEngine On
ErrorDocument 404 /404.php
RewriteCond %{REQUEST_FILENAME} !-d
RewriteCond %{REQUEST_FILENAME} !-f
RewriteRule (.*) $1.php [L]
"#,
    )
    .expect("failed to write htaccess");

    let rules = discover_htaccess_rules(temp.path());

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].request_prefix, "/IT-NVC");
    assert_eq!(rules[0].project_prefix, "/IT-NVC");
    assert!(rules[0].extensionless_php);
    assert_eq!(rules[0].error_404.as_deref(), Some("/IT-NVC/404.php"));
}

#[test]
fn test_htaccess_parser_maps_nested_relative_front_controller() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let public = temp.path().join("demo").join("public");
    std::fs::create_dir_all(&public).expect("failed to create public dir");
    std::fs::write(
        public.join(".htaccess"),
        r#"
RewriteEngine On
RewriteCond %{REQUEST_FILENAME} !-d
RewriteCond %{REQUEST_FILENAME} !-f
RewriteRule ^ index.php [L]
Options -Indexes
"#,
    )
    .expect("failed to write htaccess");

    let rules = discover_htaccess_rules(temp.path());

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].request_prefix, "/demo/public");
    assert_eq!(
        rules[0].front_controller.as_deref(),
        Some("/demo/public/index.php")
    );
    assert!(rules[0].disable_indexes);
}

#[test]
fn test_htaccess_parser_maps_php_directory_index() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let project = temp.path().join("demo");
    std::fs::create_dir_all(&project).expect("failed to create project dir");
    std::fs::write(
        project.join(".htaccess"),
        "DirectoryIndex home.php index.html index.php\n",
    )
    .expect("failed to write htaccess");

    let rules = discover_htaccess_rules(temp.path());

    assert_eq!(rules.len(), 1);
    assert_eq!(
        rules[0].directory_indexes,
        vec!["home.php".to_string(), "index.php".to_string()]
    );
}

#[test]
fn test_generate_caddyfile_includes_htaccess_compatibility_rules() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let projects = temp.path().join("projects");
    let config = temp.path().join("config");
    let logs = temp.path().join("logs");
    let adminer = temp.path().join("adminer");
    let project = projects.join("IT-NVC");
    std::fs::create_dir_all(&project).expect("failed to create project dir");
    std::fs::create_dir_all(&config).expect("failed to create config dir");
    std::fs::create_dir_all(&logs).expect("failed to create logs dir");
    std::fs::create_dir_all(&adminer).expect("failed to create adminer dir");
    std::fs::write(
        project.join(".htaccess"),
        "RewriteEngine On\nRewriteRule (.*) $1.php [L]\nErrorDocument 404 /404.php\n",
    )
    .expect("failed to write htaccess");

    let runtime_paths = RuntimePaths {
        caddy: temp.path().join("caddy"),
        php_cgi: temp.path().join("php-cgi"),
        php_ini: temp.path().join("php.ini"),
        mysql: temp.path().join("mysql"),
        postgresql: temp.path().join("postgres"),
        adminer,
        mysql_data_dir: temp.path().join("mysql-data"),
        postgresql_data_dir: temp.path().join("postgresql-data"),
        logs_dir: logs,
        config_dir: config.clone(),
        projects_dir: projects,
    };
    let caddyfile = config.join("Caddyfile");

    generate_caddyfile(
        &caddyfile,
        &runtime_paths,
        8080,
        9000,
        &config.join("welcome"),
    )
    .expect("failed to generate Caddyfile");
    let content = std::fs::read_to_string(caddyfile).expect("failed to read Caddyfile");

    assert!(content.contains("@champWelcome"));
    assert!(content.contains("not file index.html index.htm index.php"));
    assert!(content.contains("@htaccessExtPhp_0_IT_NVC"));
    assert!(content.contains("rewrite * {file_match.relative}"));
    assert!(content.contains("@htaccessError404_0_IT_NVC"));
    assert!(content.contains("rewrite * /IT-NVC/404.php"));
}

/// Everything `generate_caddyfile` needs under one temp folder.
fn caddy_fixture(temp: &Path) -> RuntimePaths {
    let paths = RuntimePaths {
        caddy: temp.join("caddy"),
        php_cgi: temp.join("php-cgi"),
        php_ini: temp.join("php.ini"),
        mysql: temp.join("mysqld"),
        postgresql: temp.join("postgres"),
        adminer: temp.join("adminer"),
        mysql_data_dir: temp.join("mysql").join("data"),
        postgresql_data_dir: temp.join("postgresql").join("data"),
        logs_dir: temp.join("logs"),
        config_dir: temp.join("config"),
        projects_dir: temp.join("projects"),
    };
    for dir in [
        &paths.adminer,
        &paths.logs_dir,
        &paths.config_dir,
        &paths.projects_dir,
    ] {
        std::fs::create_dir_all(dir).expect("failed to create fixture dir");
    }
    paths
}

fn generated_caddyfile(paths: &RuntimePaths) -> String {
    let caddyfile = paths.config_dir.join("Caddyfile");
    generate_caddyfile(
        &caddyfile,
        paths,
        8080,
        9000,
        &paths.config_dir.join("welcome"),
    )
    .expect("failed to generate Caddyfile");
    std::fs::read_to_string(caddyfile).expect("failed to read Caddyfile")
}

#[test]
fn caddyfile_binds_loopback_and_gates_database_tools_first() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let content = generated_caddyfile(&caddy_fixture(temp.path()));

    assert!(content.contains("bind 127.0.0.1"));
    assert!(content.contains("not host localhost 127.0.0.1 *.localhost"));
    assert!(content.contains("header X-Forwarded-For *"));

    let first_handle = content.find("handle ").expect("no handle block");
    assert_eq!(
        first_handle,
        content.find("handle @dbToolsNotLocal").unwrap(),
        "the database tool gate must be the first handle"
    );
    assert!(
        content.find("handle @dbToolsForwarded").unwrap()
            < content.find("handle_path /adminer/*").unwrap()
    );
}

#[test]
fn caddyfile_hides_dotfiles_except_well_known() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let content = generated_caddyfile(&caddy_fixture(temp.path()));

    assert!(content.contains("path */.*"));
    assert!(content.contains("not path */.well-known/*"));
    assert!(
        content.find("handle @dotfiles").unwrap() < content.find("handle @phpFile").unwrap(),
        "dotfiles must be refused before any PHP handler can run them"
    );
}

#[test]
fn htaccess_deny_refuses_php_before_php_handlers() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let paths = caddy_fixture(temp.path());
    let uploads = paths.projects_dir.join("shop").join("uploads");
    std::fs::create_dir_all(&uploads).unwrap();
    std::fs::write(uploads.join(".htaccess"), "Deny from all\n").unwrap();

    let content = generated_caddyfile(&paths);

    let deny = content
        .find("handle @htaccessDeny_0_shop_uploads")
        .expect("deny rule must be a handle block");
    assert!(deny < content.find("handle @phpExtensionless").unwrap());
    assert!(deny < content.find("handle @phpFile").unwrap());
    assert!(!content.contains("respond @htaccessDeny"));
}

#[test]
fn htaccess_root_deny_keeps_database_tools_reachable() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let paths = caddy_fixture(temp.path());
    std::fs::write(paths.projects_dir.join(".htaccess"), "Deny from all\n").unwrap();

    let content = generated_caddyfile(&paths);

    assert!(content.contains("not path /adminer /adminer/* /phpmyadmin /phpmyadmin/*"));
}

#[test]
fn htaccess_deny_inside_files_section_does_not_close_the_directory() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let paths = caddy_fixture(temp.path());
    let site = paths.projects_dir.join("site");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(
        site.join(".htaccess"),
        "<Files \"config.php\">\nDeny from all\n</Files>\nOptions -Indexes\n",
    )
    .unwrap();

    let rules = discover_htaccess_rules(&paths.projects_dir);

    assert_eq!(rules.len(), 1);
    assert!(!rules[0].deny_all);
    assert!(rules[0].disable_indexes);
}

#[test]
fn htaccess_deny_applies_inside_ifmodule_but_not_with_allow_exception() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let paths = caddy_fixture(temp.path());
    let closed = paths.projects_dir.join("closed");
    let local_only = paths.projects_dir.join("local");
    std::fs::create_dir_all(&closed).unwrap();
    std::fs::create_dir_all(&local_only).unwrap();
    std::fs::write(
        closed.join(".htaccess"),
        "<IfModule mod_authz_core.c>\nRequire all denied\n</IfModule>\n",
    )
    .unwrap();
    std::fs::write(
        local_only.join(".htaccess"),
        "Order Deny,Allow\nDeny from all\nAllow from 127.0.0.1\nOptions -Indexes\n",
    )
    .unwrap();

    let rules = discover_htaccess_rules(&paths.projects_dir);

    let rule = |prefix: &str| {
        rules
            .iter()
            .find(|rule| rule.request_prefix.starts_with(prefix))
            .expect("rule not found")
    };
    assert!(rule("/closed").deny_all);
    assert!(!rule("/local").deny_all);
}

#[test]
fn php_keeps_serving_past_the_default_request_limit() {
    let mut command = Command::new("php-cgi");
    apply_php_database_env(&mut command, 8080, 3306, 5432);
    let value = command
        .get_envs()
        .find(|(key, _)| *key == "PHP_FCGI_MAX_REQUESTS")
        .and_then(|(_, value)| value);
    assert_eq!(value, Some(std::ffi::OsStr::new("0")));
}

fn arg_strings(args: &[std::ffi::OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn mysql_args_ignore_global_config_and_lock_down_file_access() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let paths = caddy_fixture(temp.path());

    let args = arg_strings(&mysql_server_args(&paths, 3306, None));

    assert_eq!(args[0], "--no-defaults", "mysqld requires it to be first");
    assert!(args.contains(&"--bind-address=127.0.0.1".to_string()));
    assert!(args.contains(&"--mysqlx=OFF".to_string()));
    let secure_dir = temp.path().join("mysql").join("secure-files");
    assert!(args.contains(&format!("--secure-file-priv={}", secure_dir.display())));
    assert!(!args.contains(&"--init-file".to_string()));
}

#[test]
fn mariadb_does_not_get_the_mysql_only_x_plugin_option() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let mut paths = caddy_fixture(temp.path());
    paths.mysql = temp.path().join("mariadbd");

    let args = arg_strings(&mysql_server_args(&paths, 3306, None));

    assert!(!args.contains(&"--mysqlx=OFF".to_string()));
}

#[test]
fn mysql_socket_stays_inside_the_data_dir_and_within_the_length_limit() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let mut paths = caddy_fixture(temp.path());
    paths.mysql_data_dir = PathBuf::from("/short/data");
    if cfg!(target_os = "windows") {
        assert!(mysql_socket_path(&paths).is_none());
    } else {
        assert_eq!(
            mysql_socket_path(&paths),
            Some(PathBuf::from("/short/data/mysql.sock"))
        );
        paths.mysql_data_dir = PathBuf::from(format!("/{}", "x".repeat(120)));
        assert!(mysql_socket_path(&paths).is_none());
    }
}

#[test]
fn stale_mysql_init_files_are_removed_and_others_kept() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    for name in [
        "mysql_init_user.sql",
        "mysql_init_user_123.sql",
        "mysql.log",
        "mysql_init.log",
    ] {
        std::fs::write(temp.path().join(name), "x").unwrap();
    }

    remove_stale_mysql_init_files(temp.path());

    assert!(!temp.path().join("mysql_init_user.sql").exists());
    assert!(!temp.path().join("mysql_init_user_123.sql").exists());
    assert!(temp.path().join("mysql.log").exists());
    assert!(temp.path().join("mysql_init.log").exists());
}

#[test]
fn init_staging_moves_into_an_empty_target() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let target = temp.path().join("mysql").join("data");
    std::fs::create_dir_all(&target).unwrap();

    let staging = prepare_init_staging_dir(&target, "MySQL").unwrap();
    assert_eq!(staging, temp.path().join("mysql").join("data.initializing"));
    std::fs::create_dir_all(staging.join("mysql")).unwrap();
    promote_init_staging_dir(&staging, &target).unwrap();

    assert!(target.join("mysql").is_dir());
    assert!(!staging.exists());
}

#[test]
fn init_staging_clears_an_interrupted_attempt() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let target = temp.path().join("data");
    let leftover = temp.path().join("data.initializing");
    std::fs::create_dir_all(leftover.join("partial")).unwrap();

    let staging = prepare_init_staging_dir(&target, "MySQL").unwrap();

    assert_eq!(staging, leftover);
    assert!(!leftover.exists());
}

#[test]
fn init_staging_never_touches_a_target_with_files() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let target = temp.path().join("data");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("ibdata1"), "precious").unwrap();

    let error = prepare_init_staging_dir(&target, "MySQL").unwrap_err();

    assert!(error.contains("not a complete database"));
    assert_eq!(
        std::fs::read_to_string(target.join("ibdata1")).unwrap(),
        "precious"
    );
}

#[test]
fn fallback_port_does_not_replace_the_configured_port() {
    let mut manager = test_manager();

    manager.set_runtime_port(ServiceType::MySQL, 3308);

    assert_eq!(manager.services[&ServiceType::MySQL].port, 3308);
    assert_eq!(manager.settings.mysql_port, 3306);
    assert_eq!(
        ProcessManager::port_for_service(ServiceType::MySQL, &manager.settings),
        3306,
        "the next start must try the configured port again"
    );
}

#[test]
fn update_ports_leaves_a_running_service_on_its_real_port() {
    let mut manager = test_manager();
    manager.set_runtime_port(ServiceType::MySQL, 3308);
    manager.services.get_mut(&ServiceType::MySQL).unwrap().state = ServiceState::Running;

    let settings = crate::config::AppSettings {
        mysql_port: 3310,
        php_port: 9100,
        ..Default::default()
    };
    manager.update_ports(&settings);

    assert_eq!(manager.services[&ServiceType::MySQL].port, 3308);
    assert_eq!(manager.services[&ServiceType::PhpFpm].port, 9100);
    assert_eq!(manager.settings.mysql_port, 3310);
}

#[test]
fn saved_package_selection_reaches_the_next_stack_start() {
    let mut manager = test_manager();
    let mut selection = manager.settings.package_selection.clone();
    selection.database_engine = "postgresql".to_string();

    manager.set_package_selection(selection);

    assert_eq!(
        stack_start_services(&manager.settings.package_selection)[1],
        ServiceType::PostgreSQL
    );
}

#[test]
fn port_changes_reach_the_services_configured_with_that_port() {
    assert_eq!(
        port_dependents(ServiceType::MySQL),
        &[ServiceType::PhpFpm, ServiceType::Caddy]
    );
    assert_eq!(
        port_dependents(ServiceType::PostgreSQL),
        &[ServiceType::PhpFpm, ServiceType::Caddy]
    );
    assert_eq!(port_dependents(ServiceType::PhpFpm), &[ServiceType::Caddy]);
    assert_eq!(port_dependents(ServiceType::Caddy), &[ServiceType::PhpFpm]);
}

/// A short-lived process that exits with a failure code.
fn failing_child() -> Child {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "exit 3"]);
        command
    };
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("sh");
        command.args(["-c", "exit 3"]);
        command
    };
    command.spawn().expect("failed to spawn test process")
}

fn service_with_child(child: Child, port: u16) -> ServiceProcess {
    ServiceProcess {
        name: ServiceType::Caddy,
        child: Some(child),
        state: ServiceState::Starting,
        port,
        log_file: None,
        error_message: None,
        external_pid: None,
    }
}

#[test]
fn start_reports_a_process_that_dies_while_loading() {
    // Nothing listens here, so only the exit can end the wait early.
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let free_port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut service = service_with_child(failing_child(), free_port);

    let error = wait_until_ready(&mut service, Duration::from_secs(20)).unwrap_err();

    assert!(error.contains("exited during start"), "{error}");
    assert!(error.contains("exit code 3"), "{error}");
    assert!(service.child.is_none());
}

#[test]
fn start_returns_as_soon_as_the_port_accepts() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    // Long enough to outlive the readiness check.
    #[cfg(windows)]
    let child = Command::new("ping")
        .args(["-n", "30", "127.0.0.1"])
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to spawn test process");
    #[cfg(unix)]
    let child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("failed to spawn test process");
    let mut service = service_with_child(child, port);

    let started = Instant::now();
    let result = wait_until_ready(&mut service, Duration::from_secs(20));
    let elapsed = started.elapsed();

    let mut child = service.child.take().expect("child must be kept");
    let _ = child.kill();
    let _ = child.wait();
    assert!(result.is_ok());
    assert!(elapsed < Duration::from_secs(10), "took {elapsed:?}");
}

#[test]
fn readiness_wait_gives_up_without_failing_a_slow_service() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let free_port = listener.local_addr().unwrap().port();
    drop(listener);
    #[cfg(windows)]
    let child = Command::new("ping")
        .args(["-n", "30", "127.0.0.1"])
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to spawn test process");
    #[cfg(unix)]
    let child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("failed to spawn test process");
    let mut service = service_with_child(child, free_port);

    let result = wait_until_ready(&mut service, Duration::from_millis(200));

    let mut child = service
        .child
        .take()
        .expect("slow service must stay attached");
    let _ = child.kill();
    let _ = child.wait();
    assert!(result.is_ok());
}

#[test]
fn log_tail_survives_bytes_that_are_not_utf8() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let log = temp.path().join("mysql.log");
    std::fs::write(&log, b"first\npath \xE0\xB8 broken\nlast\n").unwrap();

    let tail = read_log_tail(&log, 2).expect("tail must not be lost");

    assert!(tail.contains("broken"));
    assert!(tail.ends_with("last"));
    assert!(!tail.contains("first"));
}

#[test]
fn lsof_field_output_matches_the_exact_executable() {
    let output = "p123\nftxt\nn/opt/champ/bin/mysqld\nftxt\nn/usr/lib/dyld\n";
    assert!(lsof_lists_executable(output, "/opt/champ/bin/mysqld"));
    assert!(!lsof_lists_executable(output, "/usr/local/bin/mysqld"));
    assert!(!lsof_lists_executable(
        "p123\nfcwd\n",
        "/opt/champ/bin/mysqld"
    ));
}

#[test]
fn php_error_log_rotates_only_past_its_limit() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let log = temp.path().join("php-errors.log");
    std::fs::write(&log, "12345").unwrap();

    rotate_php_error_log(&log, 10);
    assert!(log.exists());

    rotate_php_error_log(&log, 4);
    assert!(!log.exists());
    assert_eq!(
        std::fs::read_to_string(temp.path().join("php-errors.log.1")).unwrap(),
        "12345"
    );
}

#[test]
fn test_multiple_services_have_independent_states() {
    let mut manager = test_manager();

    let caddy = manager.services.get_mut(&ServiceType::Caddy).unwrap();
    caddy.state = ServiceState::Running;

    let php = manager.services.get_mut(&ServiceType::PhpFpm).unwrap();
    php.state = ServiceState::Starting;

    let mysql = manager.services.get_mut(&ServiceType::MySQL).unwrap();
    mysql.state = ServiceState::Stopped;

    assert_eq!(manager.status(ServiceType::Caddy), ServiceState::Running);
    assert_eq!(manager.status(ServiceType::PhpFpm), ServiceState::Starting);
    assert_eq!(manager.status(ServiceType::MySQL), ServiceState::Stopped);
}

#[test]
fn test_all_services_are_keyed_by_their_own_type() {
    let manager = test_manager();

    for (service_type, process) in &manager.services {
        assert_eq!(process.name, *service_type);
    }
}
