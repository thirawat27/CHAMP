//! `.htaccess` discovery/parsing and Caddyfile-fragment writers.
//!
//! Moved verbatim from the former monolithic `manager.rs` (code-health M-01).

use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct HtaccessCompatibilityRule {
    pub(super) request_prefix: String,
    pub(super) project_prefix: String,
    pub(super) extensionless_php: bool,
    pub(super) front_controller: Option<String>,
    pub(super) error_404: Option<String>,
    pub(super) directory_indexes: Vec<String>,
    pub(super) deny_all: bool,
    pub(super) disable_indexes: bool,
}

pub(super) fn discover_htaccess_rules(projects_dir: &Path) -> Vec<HtaccessCompatibilityRule> {
    let mut files = Vec::new();
    collect_htaccess_files(projects_dir, &mut files);
    files.sort();

    let mut rules: Vec<HtaccessCompatibilityRule> = files
        .iter()
        .filter_map(|path| parse_htaccess_file(projects_dir, path))
        .collect();

    rules.sort_by(|a, b| {
        b.request_prefix
            .len()
            .cmp(&a.request_prefix.len())
            .then_with(|| a.request_prefix.cmp(&b.request_prefix))
    });
    rules
}

// R-10: Bound the .htaccess scan so it cannot walk forever or waste time.
// `path.is_dir()` follows symlinks, so a symlink cycle could recurse until the
// stack overflows; using the DirEntry's own file type avoids following the
// entry, and skipping symlinks entirely prevents cycles. A depth cap and a
// dependency/VCS directory skip-list keep the walk cheap on real projects.
pub(super) const MAX_HTACCESS_DEPTH: usize = 12;

pub(super) const HTACCESS_SKIP_DIRS: [&str; 6] = [
    "node_modules",
    "vendor",
    ".git",
    ".svn",
    ".hg",
    "bower_components",
];

pub(super) fn collect_htaccess_files(dir: &Path, files: &mut Vec<PathBuf>) {
    collect_htaccess_files_inner(dir, files, 0);
}

pub(super) fn collect_htaccess_files_inner(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > MAX_HTACCESS_DEPTH {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        // Never follow symlinks (avoids cycles and escaping the project tree).
        if file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name();

        if file_type.is_dir() {
            let skip = HTACCESS_SKIP_DIRS
                .iter()
                .any(|skip| name.to_string_lossy().eq_ignore_ascii_case(skip));
            if skip {
                continue;
            }
            collect_htaccess_files_inner(&entry.path(), files, depth + 1);
        } else if name.to_string_lossy().eq_ignore_ascii_case(".htaccess") {
            files.push(entry.path());
        }
    }
}

pub(super) fn parse_htaccess_file(
    projects_dir: &Path,
    path: &Path,
) -> Option<HtaccessCompatibilityRule> {
    let htaccess_dir = path.parent()?;
    let relative_dir = htaccess_dir.strip_prefix(projects_dir).ok()?;
    let request_prefix = request_prefix_from_relative_path(relative_dir);
    let project_prefix = project_prefix_from_relative_path(relative_dir);
    let content = fs::read_to_string(path).ok()?;

    let mut rule = HtaccessCompatibilityRule {
        request_prefix,
        project_prefix,
        ..Default::default()
    };

    for raw_line in content.lines() {
        let line = strip_htaccess_comment(raw_line);
        if line.is_empty() {
            continue;
        }

        let lower = line.to_ascii_lowercase();
        if lower.contains("deny from all") || lower.contains("require all denied") {
            rule.deny_all = true;
            continue;
        }

        if lower.starts_with("options") && lower.contains("-indexes") {
            rule.disable_indexes = true;
            continue;
        }

        if lower.starts_with("errordocument") {
            if let Some(target) = parse_error_document_404(&line) {
                rule.error_404 = Some(resolve_htaccess_target(
                    &target,
                    &rule.request_prefix,
                    &rule.project_prefix,
                ));
            }
            continue;
        }

        if lower.starts_with("directoryindex") {
            rule.directory_indexes
                .extend(parse_php_directory_indexes(&line));
            continue;
        }

        if lower.starts_with("rewriterule") {
            if let Some(replacement) = rewrite_rule_replacement(&line) {
                let replacement_lower = replacement.to_ascii_lowercase();
                if is_front_controller_rewrite(&replacement_lower) {
                    rule.front_controller = Some(resolve_htaccess_target(
                        &replacement,
                        &rule.request_prefix,
                        &rule.project_prefix,
                    ));
                } else if is_extensionless_php_rewrite(&replacement_lower) {
                    rule.extensionless_php = true;
                }
            }
        }
    }

    if rule.extensionless_php
        || rule.front_controller.is_some()
        || rule.error_404.is_some()
        || !rule.directory_indexes.is_empty()
        || rule.deny_all
        || rule.disable_indexes
    {
        Some(rule)
    } else {
        None
    }
}

pub(super) fn strip_htaccess_comment(line: &str) -> String {
    line.split_once('#')
        .map(|(before, _)| before)
        .unwrap_or(line)
        .trim()
        .to_string()
}

pub(super) fn request_prefix_from_relative_path(relative_dir: &Path) -> String {
    let parts: Vec<String> = relative_dir
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .filter(|part| !part.is_empty())
        .collect();

    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

pub(super) fn project_prefix_from_relative_path(relative_dir: &Path) -> String {
    relative_dir
        .components()
        .next()
        .map(|component| {
            format!(
                "/{}",
                component.as_os_str().to_string_lossy().replace('\\', "/")
            )
        })
        .unwrap_or_else(|| "/".to_string())
}

pub(super) fn rewrite_rule_replacement(line: &str) -> Option<String> {
    line.split_whitespace()
        .nth(2)
        .map(|value| value.trim_matches(['"', '\'']).to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn is_extensionless_php_rewrite(replacement: &str) -> bool {
    replacement.contains("$1.php")
        || replacement.contains("${1}.php")
        || replacement.ends_with(".php")
}

pub(super) fn is_front_controller_rewrite(replacement: &str) -> bool {
    let replacement = replacement.trim_start_matches('/');
    replacement == "index.php"
        || replacement.starts_with("index.php?")
        || replacement.ends_with("/index.php")
        || replacement.contains("/index.php?")
}

pub(super) fn parse_error_document_404(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    let directive = parts.next()?;
    if !directive.eq_ignore_ascii_case("errordocument") {
        return None;
    }

    let status = parts.next()?;
    if status != "404" {
        return None;
    }

    let mut target = parts.collect::<Vec<&str>>().join(" ");
    if target.starts_with("/ ") {
        target = format!("/{}", target.trim_start_matches("/ ").trim_start());
    }

    target = target.trim_matches(['"', '\'']).trim().to_string();
    if target.is_empty() || target.starts_with("http://") || target.starts_with("https://") {
        None
    } else {
        Some(target)
    }
}

pub(super) fn parse_php_directory_indexes(line: &str) -> Vec<String> {
    line.split_whitespace()
        .skip(1)
        .map(|value| value.trim_matches(['"', '\'']).replace('\\', "/"))
        .filter(|value| value.to_ascii_lowercase().ends_with(".php"))
        .collect()
}

pub(super) fn resolve_htaccess_target(
    target: &str,
    request_prefix: &str,
    project_prefix: &str,
) -> String {
    let normalized = target.replace('\\', "/");
    if normalized.starts_with('/') {
        let trimmed_project_prefix = trim_slash(project_prefix);
        if project_prefix == "/"
            || normalized == trimmed_project_prefix
            || normalized.starts_with(&format!("{}/", trimmed_project_prefix))
        {
            normalized
        } else {
            join_url_path(project_prefix, normalized.trim_start_matches('/'))
        }
    } else {
        join_url_path(request_prefix, &normalized)
    }
}

pub(super) fn trim_slash(value: &str) -> &str {
    value.trim_end_matches('/')
}

pub(super) fn join_url_path(prefix: &str, suffix: &str) -> String {
    let suffix = suffix.trim_start_matches('/');
    if prefix == "/" {
        format!("/{}", suffix)
    } else if suffix.is_empty() {
        prefix.to_string()
    } else {
        format!("{}/{}", prefix.trim_end_matches('/'), suffix)
    }
}

pub(super) fn caddy_path_matcher_line(prefix: &str) -> Option<String> {
    if prefix == "/" {
        None
    } else {
        Some(format!(
            "        path {} {}\n",
            caddy_token(prefix),
            caddy_token(&format!("{}/*", prefix.trim_end_matches('/')))
        ))
    }
}

pub(super) fn caddy_token(value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

pub(super) fn caddy_matcher_name(prefix: &str, index: usize, suffix: &str) -> String {
    let mut sanitized = String::new();
    for character in prefix.chars() {
        if character.is_ascii_alphanumeric() {
            sanitized.push(character);
        } else if !sanitized.ends_with('_') {
            sanitized.push('_');
        }
    }
    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        format!("htaccess{}_{}", suffix, index)
    } else {
        format!("htaccess{}_{}_{}", suffix, index, sanitized)
    }
}

pub(super) fn append_htaccess_route_rules(
    content: &mut String,
    rules: &[HtaccessCompatibilityRule],
    php_port: u16,
) {
    if rules.is_empty() {
        return;
    }

    content.push_str("    # Apache .htaccess compatibility: project-specific rewrites\n");
    for (index, rule) in rules.iter().enumerate() {
        if rule.deny_all {
            let matcher = caddy_matcher_name(&rule.request_prefix, index, "Deny");
            content.push_str(&format!("    @{} {{\n", matcher));
            if let Some(path_line) = caddy_path_matcher_line(&rule.request_prefix) {
                content.push_str(&path_line);
            }
            content.push_str("    }\n");
            content.push_str(&format!("    respond @{} 403\n", matcher));
            content.push('\n');
        }

        if rule.extensionless_php {
            let matcher = caddy_matcher_name(&rule.request_prefix, index, "ExtPhp");
            content.push_str(&format!("    @{} {{\n", matcher));
            if let Some(path_line) = caddy_path_matcher_line(&rule.request_prefix) {
                content.push_str(&path_line);
            }
            content.push_str("        not path */ *.php\n");
            content.push_str("        file {\n");
            content.push_str("            try_files {path}.php\n");
            content.push_str("        }\n");
            content.push_str("    }\n");
            content.push_str(&format!("    handle @{} {{\n", matcher));
            content.push_str("        rewrite * {file_match.relative}\n");
            content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
            content.push_str("    }\n");
            content.push('\n');
        }

        if !rule.directory_indexes.is_empty() {
            let matcher = caddy_matcher_name(&rule.request_prefix, index, "Index");
            content.push_str(&format!("    @{} {{\n", matcher));
            if rule.request_prefix == "/" {
                content.push_str("        path */\n");
            } else {
                let prefix = rule.request_prefix.trim_end_matches('/');
                content.push_str(&format!(
                    "        path {} {}\n",
                    caddy_token(&format!("{}/", prefix)),
                    caddy_token(&format!("{}/*/", prefix))
                ));
            }
            content.push_str("        file {\n");
            content.push_str("            try_files");
            for index_file in &rule.directory_indexes {
                content.push_str(&format!(" {{path}}/{}", index_file));
                content.push_str(&format!(" {{path}}{}", index_file));
            }
            content.push('\n');
            content.push_str("        }\n");
            content.push_str("    }\n");
            content.push_str(&format!("    handle @{} {{\n", matcher));
            content.push_str("        rewrite * {file_match.relative}\n");
            content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
            content.push_str("    }\n");
            content.push('\n');
        }

        if let Some(front_controller) = &rule.front_controller {
            let matcher = caddy_matcher_name(&rule.request_prefix, index, "Front");
            content.push_str(&format!("    @{} {{\n", matcher));
            if let Some(path_line) = caddy_path_matcher_line(&rule.request_prefix) {
                content.push_str(&path_line);
            }
            content.push_str("        not path */ *.php\n");
            content.push_str("        not file {\n");
            content.push_str("            try_files {path} {path}/\n");
            content.push_str("        }\n");
            content.push_str("    }\n");
            content.push_str(&format!("    handle @{} {{\n", matcher));
            content.push_str(&format!(
                "        rewrite * {}\n",
                caddy_token(front_controller)
            ));
            content.push_str(&format!("        php_fastcgi 127.0.0.1:{}\n", php_port));
            content.push_str("    }\n");
            content.push('\n');
        }
    }
}

pub(super) fn append_htaccess_error_rules(
    content: &mut String,
    rules: &[HtaccessCompatibilityRule],
    php_port: u16,
) {
    let error_rules: Vec<&HtaccessCompatibilityRule> = rules
        .iter()
        .filter(|rule| rule.error_404.is_some())
        .collect();
    if error_rules.is_empty() {
        return;
    }

    content.push_str("    # Apache .htaccess compatibility: ErrorDocument mappings\n");
    content.push_str("    handle_errors {\n");
    for (index, rule) in error_rules.iter().enumerate() {
        let matcher = caddy_matcher_name(&rule.request_prefix, index, "Error404");
        content.push_str(&format!("        @{} {{\n", matcher));
        content.push_str("            expression {err.status_code} == 404\n");
        if let Some(path_line) = caddy_path_matcher_line(&rule.request_prefix) {
            content.push_str(&path_line.replace("        path", "            path"));
        }
        content.push_str("        }\n");
        content.push_str(&format!("        handle @{} {{\n", matcher));
        let error_404 = rule.error_404.as_ref().expect("filtered error_404 rule");
        content.push_str(&format!(
            "            rewrite * {}\n",
            caddy_token(error_404)
        ));
        if error_404.to_ascii_lowercase().ends_with(".php") {
            content.push_str(&format!("            php_fastcgi 127.0.0.1:{}\n", php_port));
        } else {
            content.push_str("            file_server\n");
        }
        content.push_str("        }\n");
    }
    content.push_str("        respond \"{err.status_code} {err.status_text}\" {err.status_code}\n");
    content.push_str("    }\n");
    content.push('\n');
}

pub(super) fn append_htaccess_no_browse_rules(
    content: &mut String,
    rules: &[HtaccessCompatibilityRule],
) {
    let no_browse_rules: Vec<&HtaccessCompatibilityRule> =
        rules.iter().filter(|rule| rule.disable_indexes).collect();
    if no_browse_rules.is_empty() {
        return;
    }

    content.push_str("    # Apache .htaccess compatibility: Options -Indexes\n");
    for (index, rule) in no_browse_rules.iter().enumerate() {
        let matcher = caddy_matcher_name(&rule.request_prefix, index, "NoBrowse");
        content.push_str(&format!("    @{} {{\n", matcher));
        if rule.request_prefix == "/" {
            content.push_str("        path */\n");
        } else {
            let prefix = rule.request_prefix.trim_end_matches('/');
            content.push_str(&format!(
                "        path {} {} {}\n",
                caddy_token(prefix),
                caddy_token(&format!("{}/", prefix)),
                caddy_token(&format!("{}/*/", prefix))
            ));
        }
        content.push_str("    }\n");
        content.push_str(&format!("    respond @{} 403\n", matcher));
        content.push('\n');
    }
}
