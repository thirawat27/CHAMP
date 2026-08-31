//! Database admin tool provisioning (phpMyAdmin/Adminer) helpers.
//!
//! Moved verbatim from the former monolithic `manager.rs` (code-health M-01).

use super::*;

pub(super) fn ensure_database_tool(
    paths: &RuntimePaths,
    web_port: u16,
    mysql_port: u16,
    postgresql_port: u16,
    database_tool_id: &str,
) -> Result<(), String> {
    // R-09: Reinstalling the tool (phpMyAdmin is thousands of files) on every
    // Caddy start wastes time and wipes user edits. A marker file records which
    // tool id was installed; if it matches, skip the wipe+copy. phpMyAdmin's
    // config still gets rewritten because it depends on ports that may change.
    let marker_path = paths.adminer.join(".champ-db-tool");
    if paths.adminer.exists() {
        if let Ok(existing) = fs::read_to_string(&marker_path) {
            if existing.trim() == database_tool_id {
                if database_tool_id.starts_with("phpmyadmin") {
                    write_phpmyadmin_config(&paths.adminer, web_port, mysql_port)?;
                }
                return Ok(());
            }
        }
    }

    if paths.adminer.exists() {
        fs::remove_dir_all(&paths.adminer)
            .map_err(|e| format!("Failed to reset database tool directory: {}", e))?;
    }
    fs::create_dir_all(&paths.adminer)
        .map_err(|e| format!("Failed to create database tool directory: {}", e))?;

    let index_path = paths.adminer.join("index.php");
    if let Some(source) = find_database_tool_source(paths, database_tool_id) {
        if source.is_file() {
            fs::copy(&source, &index_path).map_err(|e| {
                format!(
                    "Failed to install database tool from {}: {}",
                    source.display(),
                    e
                )
            })?;
        } else {
            copy_dir_contents(&source, &paths.adminer)?;
        }
        if database_tool_id.starts_with("phpmyadmin") {
            write_phpmyadmin_config(&paths.adminer, web_port, mysql_port)?;
        }
        // Only record the marker for a real install so placeholder installs are
        // retried on the next start until the real tool source exists.
        fs::write(&marker_path, database_tool_id)
            .map_err(|e| format!("Failed to write database tool marker: {}", e))?;
        return Ok(());
    }

    let tool_name = if database_tool_id.starts_with("adminer") {
        "Adminer"
    } else {
        "phpMyAdmin"
    };
    let tool_path = if database_tool_id.starts_with("adminer") {
        "/adminer"
    } else {
        "/phpmyadmin"
    };
    let default_connection = if database_tool_id.starts_with("adminer") {
        format!(
            "Default PostgreSQL connection: server <code>127.0.0.1:{}</code>, user <code>postgres</code>, empty password.",
            postgresql_port
        )
    } else {
        format!(
            "Default MySQL connection: server <code>127.0.0.1:{}</code>, user <code>root</code>, empty password.",
            mysql_port
        )
    };

    let placeholder = format!(
        r#"<?php
http_response_code(503);
header('Content-Type: text/html; charset=utf-8');
?>
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>{tool_name} is not installed</title>
  <style>
    body {{ font-family: system-ui, -apple-system, Segoe UI, sans-serif; margin: 48px; line-height: 1.5; }}
    code {{ background: #f3f4f6; padding: 2px 6px; border-radius: 4px; }}
  </style>
</head>
<body>
  <h1>{tool_name} is not installed</h1>
  <p>Run the CHAMP runtime installer to install {tool_name}. After installation, open <code>{tool_path}</code> again.</p>
  <p>{default_connection}</p>
</body>
</html>
"#,
        default_connection = default_connection,
        tool_name = tool_name,
        tool_path = tool_path
    );

    fs::write(&index_path, placeholder)
        .map_err(|e| format!("Failed to create database tool placeholder: {}", e))?;

    Ok(())
}

pub(super) fn find_database_tool_source(
    paths: &RuntimePaths,
    database_tool_id: &str,
) -> Option<PathBuf> {
    let mut roots = Vec::new();

    if let Some(base_dir) = paths.config_dir.parent() {
        roots.push(base_dir.join("runtime"));
    }

    if let Some(caddy_dir) = paths.caddy.parent() {
        roots.push(caddy_dir.to_path_buf());
        if let Some(parent) = caddy_dir.parent() {
            roots.push(parent.to_path_buf());
        }
    }

    for root in roots {
        if !root.exists() {
            continue;
        }

        let direct_candidates = if database_tool_id.starts_with("adminer") {
            vec![
                root.join("adminer").join("index.php"),
                root.join("adminer.php"),
            ]
        } else {
            vec![root.join("phpmyadmin")]
        };

        for candidate in direct_candidates {
            if candidate.is_file() || candidate.join("index.php").exists() {
                return Some(candidate);
            }
        }

        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();

                if database_tool_id.starts_with("adminer")
                    && path.is_file()
                    && name.starts_with("adminer")
                    && name.ends_with(".php")
                {
                    return Some(path);
                }

                if database_tool_id.starts_with("phpmyadmin")
                    && path.is_dir()
                    && name.starts_with("phpmyadmin")
                    && path.join("index.php").exists()
                {
                    return Some(path);
                }
            }
        }
    }

    None
}

pub(super) fn copy_dir_contents(source: &PathBuf, target: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| {
        format!(
            "Failed to create database tool target {}: {}",
            target.display(),
            e
        )
    })?;

    for entry in fs::read_dir(source).map_err(|e| {
        format!(
            "Failed to read database tool source {}: {}",
            source.display(),
            e
        )
    })? {
        let entry = entry.map_err(|e| format!("Failed to read database tool entry: {}", e))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_contents(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|e| {
                format!(
                    "Failed to copy database tool file {} to {}: {}",
                    source_path.display(),
                    target_path.display(),
                    e
                )
            })?;
        }
    }

    Ok(())
}

/// Length of the phpMyAdmin blowfish secret. phpMyAdmin expects a 32-byte key.
pub(super) const BLOWFISH_SECRET_LEN: usize = 32;

/// Generate `len` random characters drawn from [A-Za-z0-9].
///
/// This does not use a dedicated RNG crate (none is a direct dependency of this
/// crate). Instead it derives entropy from the current high-resolution time,
/// the process id, and the address of a stack local, then expands that seed
/// with a small xorshift PRNG. This is not cryptographically strong, but it is
/// unique per install/run and — crucially — never a shared compile-time
/// constant. The charset excludes the single quote so the value is always safe
/// to embed in a PHP single-quoted string literal.
pub(super) fn random_alphanumeric(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    let stack_marker = 0u8;
    let stack_addr = (&stack_marker as *const u8) as u64;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    // Seed the PRNG from time + pid + stack address. Force a non-zero seed so
    // the xorshift below cannot get stuck at zero.
    let mut state = nanos ^ ((std::process::id() as u64) << 32) ^ stack_addr;
    if state == 0 {
        state = 0x9E37_79B9_7F4A_7C15;
    }

    let mut out = String::with_capacity(len);
    for _ in 0..len {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let idx = (state % CHARSET.len() as u64) as usize;
        out.push(CHARSET[idx] as char);
    }
    out
}

/// Return the per-install phpMyAdmin blowfish secret, generating and persisting
/// it on first use.
///
/// The secret is stored in the CHAMP config directory at
/// `phpmyadmin-blowfish.secret`. If that file already holds exactly
/// `BLOWFISH_SECRET_LEN` characters (after trimming) it is reused; otherwise a
/// fresh random secret is generated and written back. On any IO error we fall
/// back to a freshly generated in-memory secret — never to the old shared
/// constant. (S-04)
pub(super) fn champ_blowfish_secret() -> String {
    let config_dir = match crate::runtime::locator::get_app_data_paths() {
        Ok(paths) => paths.config_dir,
        Err(_) => return random_alphanumeric(BLOWFISH_SECRET_LEN),
    };

    let secret_path = config_dir.join("phpmyadmin-blowfish.secret");

    if let Ok(existing) = fs::read_to_string(&secret_path) {
        let trimmed = existing.trim();
        if trimmed.len() == BLOWFISH_SECRET_LEN {
            return trimmed.to_string();
        }
    }

    let secret = random_alphanumeric(BLOWFISH_SECRET_LEN);

    // Best-effort persistence. If we can't write it, still return the freshly
    // generated secret so phpMyAdmin gets a non-constant key this run.
    let _ = fs::create_dir_all(&config_dir);
    let _ = fs::write(&secret_path, &secret);

    secret
}

pub(super) fn write_phpmyadmin_config(
    target: &Path,
    web_port: u16,
    mysql_port: u16,
) -> Result<(), String> {
    // S-04: Use a per-install random secret instead of a shared hardcoded
    // constant. The charset is restricted to [A-Za-z0-9] so the value can never
    // contain a single quote that would break the PHP string literal below.
    let blowfish_secret = champ_blowfish_secret();
    let config = format!(
        r#"<?php
$cfg['blowfish_secret'] = '{}';
$cfg['PmaAbsoluteUri'] = 'http://localhost:{}/phpmyadmin/';
$i = 0;
$i++;
$cfg['Servers'][$i]['auth_type'] = 'cookie';
$cfg['Servers'][$i]['host'] = '127.0.0.1';
$cfg['Servers'][$i]['port'] = '{}';
$cfg['Servers'][$i]['AllowNoPassword'] = true;
$cfg['CheckConfigurationPermissions'] = false;
$cfg['TempDir'] = __DIR__ . '/tmp';
"#,
        blowfish_secret, web_port, mysql_port
    );

    fs::create_dir_all(target.join("tmp")).map_err(|e| {
        format!(
            "Failed to create phpMyAdmin temp directory {}: {}",
            target.join("tmp").display(),
            e
        )
    })?;
    fs::write(target.join("config.inc.php"), config)
        .map_err(|e| format!("Failed to write phpMyAdmin config: {}", e))
}
