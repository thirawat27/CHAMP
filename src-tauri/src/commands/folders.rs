//! Opening folders and terminals, and reporting where CHAMP keeps things.

use crate::runtime::downloader::RuntimeDownloader;
use crate::runtime::locator::get_app_data_paths;
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Data transfer object for application paths
#[derive(Debug, Serialize)]
pub struct AppPathsDto {
    pub base_dir: String,
    pub portable: bool,
    pub runtime_dir: String,
    pub config_dir: String,
    pub mysql_data_dir: String,
    pub postgresql_data_dir: String,
    pub logs_dir: String,
    pub projects_dir: String,
}

/// `path` with symlinks resolved as far as it exists. The rest does not exist
/// yet, so it cannot be a link.
pub(super) fn resolve_existing_prefix(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut missing = Vec::new();
    loop {
        if let Ok(resolved) = dunce::canonicalize(existing) {
            return missing
                .iter()
                .rev()
                .fold(resolved, |resolved, name| resolved.join(name));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name);
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// True when `path` is absolute, has no `..` segments, and really lies inside one
/// of `roots`: a symlink inside a root that points elsewhere does not count.
pub(super) fn is_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return false;
    }
    let resolved = resolve_existing_prefix(path);
    roots
        .iter()
        .any(|root| resolved.starts_with(resolve_existing_prefix(root)))
}

/// The UI may only open (and create) folders inside the CHAMP data folder, which
/// also holds the download cache, so a compromised webview cannot create or
/// reveal arbitrary paths through these commands.
pub(super) fn ensure_allowed_folder(path: &str) -> Result<PathBuf, String> {
    let base_dir = get_app_data_paths()?.base_dir;
    let roots = [base_dir.clone()];
    let path = PathBuf::from(path);
    if is_within_roots(&path, &roots) {
        Ok(path)
    } else {
        Err(format!(
            "CHAMP can only open folders inside {}",
            base_dir.display()
        ))
    }
}

/// Open a folder in the system's file explorer using tauri-plugin-opener.
///
/// The folder is created first when it does not exist yet.
#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    use tauri_plugin_opener::reveal_item_in_dir;

    let path = ensure_allowed_folder(&path)?;
    if !path.exists() {
        fs::create_dir_all(&path).map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    reveal_item_in_dir(&path).map_err(|e| format!("Failed to open folder: {}", e))?;

    Ok(())
}

/// Collect the bin directories of the installed CHAMP runtimes (PHP CLI,
/// MySQL/PostgreSQL clients) so a terminal can use them without setup.
pub(super) fn runtime_bin_dirs() -> Vec<PathBuf> {
    let Ok(runtimes) = crate::runtime::locator::locate_runtime_binaries() else {
        return Vec::new();
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    for binary in [&runtimes.php_cgi, &runtimes.mysql, &runtimes.postgresql] {
        if let Some(parent) = binary.parent() {
            if parent.exists() && !dirs.iter().any(|dir| dir == parent) {
                dirs.push(parent.to_path_buf());
            }
        }
    }
    dirs
}

/// Open a terminal in a specific directory with the installed runtimes on PATH.
#[tauri::command]
pub async fn open_project_terminal(path: Option<String>) -> Result<(), String> {
    let target_dir = match path {
        Some(p) => ensure_allowed_folder(&p)?,
        None => get_app_data_paths()?.projects_dir,
    };

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create terminal directory: {}", e))?;
    }

    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = runtime_bin_dirs();
    paths.extend(std::env::split_paths(&current_path));
    let new_path = std::env::join_paths(paths).unwrap_or(current_path);

    crate::process::terminal::spawn_terminal(&target_dir, &new_path)
}

/// Get the runtime directory path
#[tauri::command]
pub async fn get_runtime_dir() -> Result<String, String> {
    RuntimeDownloader::new()
        .get_runtime_dir()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_app_paths() -> Result<AppPathsDto, String> {
    let paths = get_app_data_paths()?;
    Ok(AppPathsDto {
        base_dir: paths.base_dir.to_string_lossy().to_string(),
        portable: paths.portable,
        runtime_dir: paths.runtime_dir.to_string_lossy().to_string(),
        config_dir: paths.config_dir.to_string_lossy().to_string(),
        mysql_data_dir: paths.mysql_data_dir.to_string_lossy().to_string(),
        postgresql_data_dir: paths.postgresql_data_dir.to_string_lossy().to_string(),
        logs_dir: paths.logs_dir.to_string_lossy().to_string(),
        projects_dir: paths.projects_dir.to_string_lossy().to_string(),
    })
}

/// Get the download directory path (where downloaded archives are cached)
#[tauri::command]
pub async fn get_download_dir() -> Result<String, String> {
    Ok(get_app_data_paths()?
        .downloads_dir
        .to_string_lossy()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_scope_only_allows_paths_inside_roots() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp.path().join("CHAMP");
        let roots = [root.clone()];

        assert!(is_within_roots(&root.join("logs"), &roots));
        assert!(is_within_roots(&root, &roots));
        assert!(!is_within_roots(&temp.path().join("other"), &roots));
        assert!(!is_within_roots(&root.join("..").join("other"), &roots));
        assert!(!is_within_roots(Path::new("relative/logs"), &roots));
        // A sibling whose name merely starts with the root's name is outside.
        assert!(!is_within_roots(&temp.path().join("CHAMP-evil"), &roots));
    }

    #[test]
    fn folder_scope_accepts_paths_that_do_not_exist_yet() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp.path().join("CHAMP");
        fs::create_dir_all(&root).unwrap();
        let roots = [root.clone()];

        assert!(is_within_roots(&root.join("new").join("logs"), &roots));
    }

    #[cfg(unix)]
    #[test]
    fn folder_scope_rejects_a_symlink_that_leaves_the_root() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp.path().join("CHAMP");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        let roots = [root.clone()];

        assert!(!is_within_roots(&root.join("link"), &roots));
        assert!(!is_within_roots(&root.join("link").join("new"), &roots));
    }
}
