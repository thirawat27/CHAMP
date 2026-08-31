//! Caddyfile generation.
//!
//! Moved verbatim from the former monolithic `manager.rs` (code-health M-01).

use super::*;

/// Generate a basic Caddyfile
pub(super) fn generate_caddyfile(
    path: &PathBuf,
    paths: &RuntimePaths,
    port: u16,
    php_port: u16,
) -> Result<(), String> {
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
    content.push_str("    # Apache .htaccess compatibility: never serve control files\n");
    content.push_str("    @apacheControlFiles {\n");
    content.push_str("        path .htaccess .htpasswd */.htaccess */.htpasswd\n");
    content.push_str("    }\n");
    content.push_str("    respond @apacheControlFiles 404\n");
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
