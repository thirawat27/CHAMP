//! Caddyfile generation.

use super::htaccess::{
    append_htaccess_deny_rules, append_htaccess_error_rules, append_htaccess_no_browse_rules,
    append_htaccess_route_rules, caddy_matcher_name, caddy_path_matcher_line, caddy_token,
    discover_htaccess_rules, HtaccessCompatibilityRule,
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

/// `<folder>.localhost` names, captured as `{re.project.1}`. Only plain DNS
/// labels match (see `crate::projects::domain_label`), so the name can never
/// carry a path separator or `..` into the folder lookup. No `{n,m}` counts:
/// braces in a Caddyfile token read as placeholders.
pub(super) const PROJECT_HOST_REGEXP: &str = "(?i)^([a-z0-9]([a-z0-9-]*[a-z0-9])?)[.]localhost$";

/// `.htaccess` "Deny from all" rules, rewritten for the domain site, where a
/// project's URLs start at `/` (or at `/` inside `public/`).
fn append_domain_deny_rules(content: &mut String, rules: &[HtaccessCompatibilityRule]) {
    for (index, rule) in rules.iter().enumerate().filter(|(_, rule)| rule.deny_all) {
        let host = if rule.project_prefix == "/" {
            // A deny at the projects root covers every project.
            None
        } else {
            match crate::projects::domain_label(rule.project_prefix.trim_start_matches('/')) {
                Some(label) => Some(format!("{label}.localhost")),
                None => continue, // no domain for this folder
            }
        };
        let relative = rule
            .request_prefix
            .strip_prefix(rule.project_prefix.trim_end_matches('/'))
            .unwrap_or("")
            .to_string();
        let mut prefixes = vec![relative.clone()];
        if let Some(inside_public) =
            relative.strip_prefix(&format!("/{}", crate::projects::PUBLIC_DIR))
        {
            if inside_public.is_empty() || inside_public.starts_with('/') {
                prefixes.push(inside_public.to_string());
            }
        }
        for (variant, prefix) in prefixes.iter().enumerate() {
            let matcher =
                caddy_matcher_name(&rule.request_prefix, index * 2 + variant, "DomainDeny");
            content.push_str(&format!("    @{} {{\n", matcher));
            if let Some(host) = &host {
                content.push_str(&format!("        host {}\n", host));
            }
            match caddy_path_matcher_line(if prefix.is_empty() { "/" } else { prefix }) {
                Some(path_line) => content.push_str(&path_line),
                None => content.push_str("        path /*\n"),
            }
            content.push_str("    }\n");
            content.push_str(&format!("    handle @{} {{\n", matcher));
            content.push_str("        respond 403\n");
            content.push_str("    }\n\n");
        }
    }
}

/// `.htaccess` front controllers (`RewriteRule ^ index.php`), rewritten for
/// each project's domain: WordPress permalinks and routers in a sub-folder
/// keep working there without a blind `index.php` fallback. The domain
/// serves `public/` when it has an index, so rules are made relative to that
/// web root, and rules outside it are skipped (they are unreachable there).
fn append_domain_route_rules(
    content: &mut String,
    rules: &[HtaccessCompatibilityRule],
    projects: &str,
    projects_dir: &Path,
    php_port: u16,
) {
    for (index, rule) in rules.iter().enumerate() {
        let Some(front_controller) = &rule.front_controller else {
            continue;
        };
        let folder = rule.project_prefix.trim_matches('/');
        // Projects-root rules have no single domain.
        if folder.is_empty() || folder.contains('/') {
            continue;
        }
        let Some(label) = crate::projects::domain_label(folder) else {
            continue;
        };
        let project_prefix = rule.project_prefix.trim_end_matches('/');
        let Some(relative) = rule.request_prefix.strip_prefix(project_prefix) else {
            continue;
        };
        let Some(target) = front_controller.strip_prefix(project_prefix) else {
            continue;
        };
        let serves_public = super::welcome::has_index_file(
            &projects_dir.join(folder).join(crate::projects::PUBLIC_DIR),
        );
        let public = format!("/{}", crate::projects::PUBLIC_DIR);
        let (relative, target, web_root) = if serves_public {
            let inside = |path: &str| {
                path.strip_prefix(&public)
                    .filter(|rest| rest.is_empty() || rest.starts_with('/'))
                    .map(|rest| if rest.is_empty() { "/" } else { rest }.to_string())
            };
            match (inside(relative), inside(target)) {
                (Some(relative), Some(target)) => (
                    relative,
                    target,
                    format!("{projects}/{folder}/{}", crate::projects::PUBLIC_DIR),
                ),
                _ => continue,
            }
        } else {
            (
                if relative.is_empty() {
                    "/".to_string()
                } else {
                    relative.to_string()
                },
                target.to_string(),
                format!("{projects}/{folder}"),
            )
        };
        let matcher = caddy_matcher_name(&rule.request_prefix, index, "DomainFront");
        content.push_str(&format!("    @{} {{\n", matcher));
        content.push_str(&format!("        host {label}.localhost\n"));
        if let Some(path_line) = caddy_path_matcher_line(&relative) {
            content.push_str(&path_line);
        }
        content.push_str("        not path */ *.php\n");
        content.push_str("        not file {\n");
        content.push_str(&format!("            root \"{web_root}\"\n"));
        content.push_str("            try_files {path} {path}/ {path}.php\n");
        content.push_str("        }\n");
        content.push_str("    }\n");
        content.push_str(&format!("    handle @{} {{\n", matcher));
        content.push_str(&format!("        root * \"{web_root}\"\n"));
        content.push_str(&format!("        rewrite * {}\n", caddy_token(&target)));
        content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
        content.push_str("    }\n\n");
    }
}

/// The `try_files` order for a domain site.
///
/// Directory indexes come before the directory itself: php_fastcgi's default
/// order sends `/` straight to PHP, which fails with "No input file
/// specified" for a site that only has `index.html`. `{path}.php` maps
/// extensionless URLs (`/view/index` → `/view/index.php`), as the path site
/// does.
pub(super) const DOMAIN_TRY_FILES: &str =
    "{path}/index.php {path}/index.html {path}/index.htm {path} {path}.php";

/// PHP for a domain site; see [`DOMAIN_TRY_FILES`].
///
/// `front_controller_fallback` sends unknown paths to `index.php`. Only a
/// `public/` web root gets it: that layout is the front-controller convention
/// (Laravel, Symfony, Slim). A project served from its folder root behaves
/// like Apache instead: unknown paths are a 404 unless its `.htaccess` names a
/// front controller (see `append_domain_route_rules`). Before 2.1.1 every
/// project fell back blindly, so an app whose root page redirects to a
/// relative path (`Location: view/index`) looped forever
/// (`/view/view/view/...`, ERR_TOO_MANY_REDIRECTS).
fn append_domain_php(content: &mut String, php_port: u16, front_controller_fallback: bool) {
    content.push_str(&format!(
        "            php_fastcgi 127.0.0.1:{} {{\n",
        php_port
    ));
    content.push_str(&format!(
        "                try_files {}{}\n",
        DOMAIN_TRY_FILES,
        if front_controller_fallback {
            " index.php"
        } else {
            ""
        }
    ));
    content.push_str("            }\n");
}

/// The `http://<folder>.localhost:<port>` site: every project at the root of
/// its own origin, so frameworks that expect `/` (Laravel, WordPress) work
/// without a sub-path. A folder with `public/index.php` (or `.html`) is served
/// from `public/`. The folder is looked up per request, so new projects need
/// no restart.
fn append_project_domains_site(
    content: &mut String,
    projects: &str,
    projects_dir: &Path,
    port: u16,
    php_port: u16,
    log_file: &str,
    htaccess_rules: &[HtaccessCompatibilityRule],
) {
    let public = crate::projects::PUBLIC_DIR;
    content.push_str(
        "\n# Project domains: http://<folder>.localhost (browsers resolve *.localhost to\n",
    );
    content.push_str("# this machine, so no hosts file and no administrator rights are needed)\n");
    content.push_str(&format!("http://*.localhost:{} {{\n", port));
    content.push_str("    bind 127.0.0.1\n\n");
    content.push_str("    # Never serve dotfiles (.env, .git, .htaccess, ...)\n");
    content.push_str("    @dotfiles {\n");
    content.push_str("        path */.*\n");
    content.push_str("        not path */.well-known/*\n");
    content.push_str("    }\n");
    content.push_str("    handle @dotfiles {\n");
    content.push_str("        respond 404\n");
    content.push_str("    }\n\n");
    append_domain_deny_rules(content, htaccess_rules);
    append_domain_route_rules(content, htaccess_rules, projects, projects_dir, php_port);
    content.push_str(&format!(
        "    @project vars_regexp project {{host}} \"{}\"\n",
        PROJECT_HOST_REGEXP
    ));
    content.push_str("    handle @project {\n");
    content.push_str("        @publicRoot file {\n");
    content.push_str(&format!(
        "            root \"{}/{{re.project.1}}/{}\"\n",
        projects, public
    ));
    content.push_str("            try_files /index.php /index.html\n");
    content.push_str("        }\n");
    content.push_str("        handle @publicRoot {\n");
    content.push_str(&format!(
        "            root * \"{}/{{re.project.1}}/{}\"\n",
        projects, public
    ));
    append_domain_php(content, php_port, true);
    content.push_str("            file_server\n");
    content.push_str("        }\n");
    content.push_str("        handle {\n");
    content.push_str(&format!(
        "            root * \"{}/{{re.project.1}}\"\n",
        projects
    ));
    append_domain_php(content, php_port, false);
    content.push_str("            file_server\n");
    content.push_str("        }\n");
    content.push_str("    }\n\n");
    content.push_str("    handle {\n");
    content
        .push_str("        respond \"Use a project folder name: http://<folder>.localhost\" 404\n");
    content.push_str("    }\n\n");
    content.push_str("    log {\n");
    content.push_str(&format!("        output file \"{}\"\n", log_file));
    content.push_str("        format json\n");
    content.push_str("    }\n");
    content.push_str("    encode gzip\n");
    content.push_str("    header {\n");
    content.push_str("        X-Content-Type-Options nosniff\n");
    content.push_str("        X-Frame-Options SAMEORIGIN\n");
    content.push_str("        Referrer-Policy no-referrer\n");
    content.push_str("    }\n");
    content.push_str("}\n");
}

/// Generate a basic Caddyfile
pub(super) fn generate_caddyfile(
    path: &PathBuf,
    paths: &RuntimePaths,
    port: u16,
    php_port: u16,
    welcome_dir: &Path,
    project_domains: bool,
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
    // The page is rewritten whenever the project folders change; never let
    // the browser show an old copy.
    content.push_str("        header Cache-Control \"no-store\"\n");
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

    // Settings → General → Project domains.
    if project_domains {
        append_project_domains_site(
            &mut content,
            &projects,
            &paths.projects_dir,
            port,
            php_port,
            &log_file,
            &htaccess_rules,
        );
    }

    let mut file = File::create(path).map_err(|e| format!("Failed to create Caddyfile: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write Caddyfile: {}", e))?;

    Ok(())
}
