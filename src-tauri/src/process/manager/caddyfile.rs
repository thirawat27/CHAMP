//! Caddyfile generation.

use super::htaccess::{
    append_htaccess_deny_rules, append_htaccess_error_rules, append_htaccess_no_browse_rules,
    append_htaccess_route_rules, discover_htaccess_rules, HtaccessCompatibilityRule,
};
use crate::runtime::locator::RuntimePaths;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Every URL path that reaches the database tools.
pub(super) const DATABASE_TOOL_PATHS: &str = "/adminer /adminer/* /phpmyadmin /phpmyadmin/*";

/// Rules that refuse a request outright. Caddy runs `handle` blocks in the order
/// they are written and before `handle_path`, `respond`, and `file_server`, so
/// these must be emitted ahead of every other `handle`.
fn append_access_gates(content: &mut String, htaccess_rules: &[HtaccessCompatibilityRule]) {
    // The database tools log in without a real password. A Host other than this
    // machine means HTTPS Preview or a DNS-rebinding page; X-Forwarded-For means
    // the request came through a proxy such as ngrok.
    content.push_str("    # Database tools are reachable from this machine only\n");
    content.push_str("    @dbToolsNotLocal {\n");
    content.push_str(&format!("        path {}\n", DATABASE_TOOL_PATHS));
    content.push_str("        not host localhost 127.0.0.1 *.localhost\n");
    content.push_str("    }\n");
    content.push_str("    handle @dbToolsNotLocal {\n");
    content.push_str("        respond 403\n");
    content.push_str("    }\n");
    content.push_str("    @dbToolsForwarded {\n");
    content.push_str(&format!("        path {}\n", DATABASE_TOOL_PATHS));
    content.push_str("        header X-Forwarded-For *\n");
    content.push_str("    }\n");
    content.push_str("    handle @dbToolsForwarded {\n");
    content.push_str("        respond 403\n");
    content.push_str("    }\n\n");

    // Matching any "/." path segment also covers .htaccess, .htpasswd, .env,
    // .git, and Windows trailing-dot spellings, for PHP as well as static files.
    content.push_str("    # Never serve dotfiles (.env, .git, .htaccess, ...)\n");
    content.push_str("    @dotfiles {\n");
    content.push_str("        path */.*\n");
    content.push_str("        not path */.well-known/*\n");
    content.push_str("    }\n");
    content.push_str("    handle @dotfiles {\n");
    content.push_str("        respond 404\n");
    content.push_str("    }\n\n");

    append_htaccess_deny_rules(content, htaccess_rules);
}

/// Generate a basic Caddyfile
pub(super) fn generate_caddyfile(
    path: &PathBuf,
    paths: &RuntimePaths,
    port: u16,
    php_port: u16,
    welcome_dir: &Path,
) -> Result<(), String> {
    let welcome = welcome_dir
        .to_str()
        .ok_or("Invalid welcome page path")?
        .replace('\\', "/");
    // Convert paths to use forward slashes for Caddyfile (cross-platform compatibility)
    let projects = paths
        .projects_dir
        .to_str()
        .ok_or("Invalid project path")?
        .replace('\\', "/");
    let log_file = paths
        .logs_dir
        .join("caddy-access.log")
        .to_str()
        .ok_or("Invalid log path")?
        .replace('\\', "/");
    let adminer = paths
        .adminer
        .to_str()
        .ok_or("Invalid Adminer path")?
        .replace('\\', "/");
    let htaccess_rules = discover_htaccess_rules(&paths.projects_dir);

    // Build the Caddyfile content
    let mut content = String::new();
    content.push_str("{\n");
    content.push_str("    admin off\n");
    content.push_str("}\n\n");
    content.push_str(&format!(":{} {{\n", port));
    // Local development only. HTTPS Preview reaches this through 127.0.0.1 too.
    content.push_str("    bind 127.0.0.1\n\n");
    append_access_gates(&mut content, &htaccess_rules);
    content.push_str("    # Database tools - must come before project root directives\n");
    content.push_str("    redir /adminer /adminer/\n");
    content.push_str("    redir /phpmyadmin /phpmyadmin/\n");
    content.push('\n');
    content.push_str("    handle_path /adminer/* {\n");
    content.push_str(&format!("        root * \"{}\"\n", adminer));
    content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
    content.push_str("        file_server\n");
    content.push_str("    }\n");
    content.push('\n');
    // NOTE: Only one database tool (phpMyAdmin OR Adminer) is installed at a time,
    // into paths.adminer. Both routes intentionally point at that single directory. (R-16)
    content.push_str("    handle_path /phpmyadmin/* {\n");
    content.push_str(&format!("        root * \"{}\"\n", adminer));
    content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
    content.push_str("        file_server\n");
    content.push_str("    }\n");
    content.push('\n');
    content.push_str("    # Root directory for serving files (default project root)\n");
    content.push_str(&format!("    root * \"{}\"\n", projects));
    content.push('\n');
    append_htaccess_route_rules(&mut content, &htaccess_rules, php_port);
    content.push_str("    # Apache .htaccess compatibility: extensionless PHP rewrites\n");
    content.push_str("    @phpExtensionless {\n");
    content.push_str("        not path */\n");
    content.push_str("        not path *.php\n");
    content.push_str("        file {\n");
    content.push_str("            try_files {path}.php\n");
    content.push_str("        }\n");
    content.push_str("    }\n");
    content.push_str("    handle @phpExtensionless {\n");
    content.push_str("        rewrite * {file_match.relative}\n");
    content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
    content.push_str("    }\n");
    content.push('\n');
    content.push_str("    # Execute PHP files only when the target script exists\n");
    content.push_str("    @phpFile {\n");
    content.push_str("        path *.php\n");
    content.push_str("        file {path}\n");
    content.push_str("    }\n");
    content.push_str("    handle @phpFile {\n");
    content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
    content.push_str("    }\n");
    content.push('\n');
    content.push_str("    # Execute directory indexes only when index.php exists\n");
    content.push_str("    @phpIndex {\n");
    content.push_str("        file {\n");
    content.push_str("            try_files {path}/index.php {path}index.php\n");
    content.push_str("        }\n");
    content.push_str("    }\n");
    content.push_str("    handle @phpIndex {\n");
    content.push_str("        rewrite * {file_match.relative}\n");
    content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
    content.push_str("    }\n");
    content.push('\n');
    // Landing page at "/" while the projects folder has no index of its own.
    // Checked per request, so dropping an index.html in the folder takes over
    // immediately without restarting Caddy.
    content.push_str("    # CHAMP landing page when the projects root has no index file\n");
    content.push_str("    @champWelcome {\n");
    content.push_str("        path /\n");
    content.push_str("        not file index.html index.htm index.php\n");
    content.push_str("    }\n");
    content.push_str("    handle @champWelcome {\n");
    content.push_str(&format!("        root * \"{}\"\n", welcome));
    content.push_str("        rewrite * /index.html\n");
    content.push_str("        file_server\n");
    content.push_str("    }\n");
    content.push('\n');
    append_htaccess_no_browse_rules(&mut content, &htaccess_rules);
    // S-03: Directory browsing is disabled. Listing the whole projects tree has
    // little value as a default and is a real exposure if the site is ever
    // fronted by the HTTPS Preview / tunnel feature.
    content
        .push_str("    # File server for project files (directory browsing disabled for safety)\n");
    content.push_str("    file_server\n");
    content.push('\n');
    append_htaccess_error_rules(&mut content, &htaccess_rules, php_port);
    content.push_str("    # Logging\n");
    content.push_str("    log {\n");
    content.push_str(&format!("        output file \"{}\"\n", log_file));
    content.push_str("        format json\n");
    content.push_str("    }\n");
    content.push('\n');
    content.push_str("    # Encode responses\n");
    content.push_str("    encode gzip\n");
    content.push('\n');
    content.push_str("    # Security headers\n");
    content.push_str("    header {\n");
    content.push_str("        X-Content-Type-Options nosniff\n");
    content.push_str("        X-Frame-Options SAMEORIGIN\n");
    content.push_str("        Referrer-Policy no-referrer\n");
    content.push_str("    }\n");
    content.push_str("}\n");

    let mut file = File::create(path).map_err(|e| format!("Failed to create Caddyfile: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write Caddyfile: {}", e))?;

    Ok(())
}
