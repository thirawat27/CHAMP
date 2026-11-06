//! Installing components into the runtime folder.

use super::fetch::PROGRESS_EMIT_INTERVAL;
use super::{
    read_marker_version, BinaryComponent, DownloadProgress, DownloadStep, ProgressCallback,
    RuntimeDownloader,
};
use crate::runtime::locator::get_app_data_paths;
use crate::runtime::packages::{is_safe_path_segment, PackageSelection};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::task::JoinSet;

impl RuntimeDownloader {
    /// Download and install all runtime binaries
    pub async fn download_all(
        &self,
        progress_cb: ProgressCallback,
    ) -> Result<Vec<PathBuf>, String> {
        self.download_and_install_components(self.selected_stack_components().to_vec(), progress_cb)
            .await
    }

    pub async fn download_all_with_skip(
        &self,
        progress_cb: ProgressCallback,
        skip_list: &[&str], // Component names to skip (e.g., ["php", "mysql"])
    ) -> Result<Vec<PathBuf>, String> {
        let components = self
            .selected_stack_components()
            .into_iter()
            .filter(|component| {
                let component_name = component.binary_name();
                let selected_database_tool_id = self.selected_database_tool_id();
                let should_skip_database_tool = *component == BinaryComponent::PhpMyAdmin
                    && ((crate::runtime::packages::is_adminer_id(&selected_database_tool_id)
                        && skip_list.contains(&"adminer"))
                        || (crate::runtime::packages::is_phpmyadmin_id(
                            &selected_database_tool_id,
                        ) && skip_list.contains(&"phpmyadmin")));

                if skip_list.contains(&component_name) || should_skip_database_tool {
                    eprintln!("Skipping {} (requested by caller)", component.name());
                    false
                } else {
                    true
                }
            })
            .collect();

        self.download_and_install_components(components, progress_cb)
            .await
    }

    pub(super) async fn download_and_install_components(
        &self,
        components: Vec<BinaryComponent>,
        progress_cb: ProgressCallback,
    ) -> Result<Vec<PathBuf>, String> {
        let total = components.len() as u8;

        if components.is_empty() {
            self.finish_download_batch(&progress_cb, total);
            return Ok(Vec::new());
        }

        let download_dir = get_app_data_paths()?.downloads_dir;
        fs::create_dir_all(&download_dir)
            .map_err(|e| format!("Failed to create download directory: {}", e))?;
        let runtime_dir = self.get_runtime_dir()?;
        fs::create_dir_all(&runtime_dir)
            .map_err(|e| format!("Failed to create runtime directory: {}", e))?;

        // --- Phase 1: Download ALL components in parallel ---
        let progress_cb = Arc::new(progress_cb);

        // Per-component atomic progress trackers: (downloaded_bytes, total_bytes)
        let per_component_downloaded: Vec<Arc<AtomicU64>> = (0..components.len())
            .map(|_| Arc::new(AtomicU64::new(0)))
            .collect();
        let per_component_total: Vec<Arc<AtomicU64>> = (0..components.len())
            .map(|_| Arc::new(AtomicU64::new(0)))
            .collect();

        // Spawn a progress aggregator task that polls all atomics and emits combined progress
        let agg_progress_cb = Arc::clone(&progress_cb);
        let agg_downloaded = per_component_downloaded.clone();
        let agg_totals = per_component_total.clone();
        let agg_total_components = total;
        let agg_cancel = tokio_util::sync::CancellationToken::new();
        let agg_cancel_clone = agg_cancel.clone();
        // Dropped on every exit, including a failed download below; a token alone
        // would leave the aggregator waking up forever.
        let aggregator_guard = agg_cancel.drop_guard();

        let aggregator_handle = tokio::spawn(async move {
            let mut last_percent = u8::MAX;
            loop {
                tokio::select! {
                    _ = agg_cancel_clone.cancelled() => break,
                    _ = tokio::time::sleep(PROGRESS_EMIT_INTERVAL) => {}
                }

                let sum_downloaded: u64 = agg_downloaded
                    .iter()
                    .map(|a| a.load(Ordering::Relaxed))
                    .sum();
                let sum_total: u64 = agg_totals.iter().map(|a| a.load(Ordering::Relaxed)).sum();
                let percent = if sum_total > 0 {
                    ((sum_downloaded as f64 / sum_total as f64) * 100.0)
                        .round()
                        .clamp(0.0, 100.0) as u8
                } else {
                    0
                };

                if percent != last_percent {
                    last_percent = percent;
                    agg_progress_cb(DownloadProgress {
                        step: DownloadStep::Downloading,
                        percent,
                        current_component: format!("All {}", agg_total_components),
                        component_display: "All Components".to_string(),
                        version: String::new(),
                        total_components: agg_total_components,
                        downloaded_bytes: sum_downloaded,
                        total_bytes: sum_total,
                    });
                }
            }
        });

        // Spawn parallel download tasks
        let mut join_set = JoinSet::new();

        for (index, &component) in components.iter().enumerate() {
            let downloader = self.clone();
            let dest_dir = download_dir.clone();
            let downloaded_atomic = Arc::clone(&per_component_downloaded[index]);
            let total_atomic = Arc::clone(&per_component_total[index]);
            let task_total = total;

            join_set.spawn(async move {
                // Per-component callback only updates atomics — aggregator handles UI
                let component_cb: ProgressCallback = Box::new(move |progress| {
                    downloaded_atomic.store(progress.downloaded_bytes, Ordering::Relaxed);
                    total_atomic.store(progress.total_bytes, Ordering::Relaxed);
                });

                let path = downloader
                    .download_component(
                        component,
                        &dest_dir,
                        &component_cb,
                        (index + 1) as u8,
                        task_total,
                    )
                    .await?;

                Ok::<(BinaryComponent, PathBuf), String>((component, path))
            });
        }

        // Collect all download results
        let mut download_results: Vec<Option<(BinaryComponent, PathBuf)>> =
            (0..components.len()).map(|_| None).collect();
        let mut completed = 0_usize;

        while let Some(result) = join_set.join_next().await {
            let (component, path) =
                result.map_err(|e| format!("Download task panicked: {}", e))??;

            // Find the index for this component
            let index = components
                .iter()
                .position(|&c| c == component)
                .ok_or_else(|| {
                    format!("Unknown component in download result: {}", component.name())
                })?;
            download_results[index] = Some((component, path));
            completed += 1;

            eprintln!(
                "[PARALLEL] Downloaded {}/{}: {}",
                completed,
                components.len(),
                component.name()
            );
        }

        // Stop the aggregator
        drop(aggregator_guard);
        let _ = aggregator_handle.await;

        // --- Phase 2: Install components sequentially ---
        let mut downloaded_files = Vec::new();

        for (index, result) in download_results.into_iter().enumerate() {
            let (component, path) = result.ok_or_else(|| {
                format!("Missing download result for component at index {}", index)
            })?;

            progress_cb(DownloadProgress {
                step: DownloadStep::Extracting,
                percent: 0,
                current_component: self.component_name(component),
                component_display: self.component_display_name(component),
                version: self.get_component_version(&component),
                total_components: total,
                downloaded_bytes: 0,
                total_bytes: 0,
            });

            let installer = self.clone();
            let install_path = path.clone();
            let install_runtime_dir = runtime_dir.clone();
            let installed = tokio::task::spawn_blocking(move || {
                installer.install_downloaded_component(
                    component,
                    &install_path,
                    &install_runtime_dir,
                )
            })
            .await
            .map_err(|e| format!("Install task failed for {}: {}", component.name(), e))?;

            // Installed: no longer needed. Failed: it may be the cause, so the next
            // attempt must fetch a fresh copy.
            let _ = fs::remove_file(&path);
            installed?;

            downloaded_files.push(path);
        }

        // Create all application directories (config, logs, database data, projects)
        if let Ok(app_paths) = get_app_data_paths() {
            if let Err(e) = app_paths.ensure_directories() {
                eprintln!("Warning: Failed to create app directories: {}", e);
            }
        }

        self.finish_download_batch(&progress_cb, total);

        Ok(downloaded_files)
    }

    pub(super) fn finish_download_batch(&self, progress_cb: &ProgressCallback, total: u8) {
        progress_cb(DownloadProgress {
            step: DownloadStep::Complete,
            percent: 100,
            current_component: "All".to_string(),
            component_display: "All Components".to_string(),
            version: String::new(),
            total_components: total,
            downloaded_bytes: 0,
            total_bytes: 0,
        });
    }

    /// Get the runtime directory
    pub fn get_runtime_dir(&self) -> Result<PathBuf, String> {
        Ok(get_app_data_paths()?.runtime_dir)
    }

    pub(super) fn install_downloaded_component(
        &self,
        component: BinaryComponent,
        downloaded_path: &Path,
        runtime_dir: &Path,
    ) -> Result<(), String> {
        if component == BinaryComponent::Php {
            return self.install_php_version(downloaded_path, runtime_dir);
        }

        // Left behind if extraction dies midway, so a partly written component is
        // not mistaken for an installed one. See `install_was_interrupted`.
        let sentinel = install_sentinel_path(runtime_dir, self.marker_name(component));
        fs::write(&sentinel, "")
            .map_err(|e| format!("Failed to prepare {} installation: {}", component.name(), e))?;

        self.extract_component(component, downloaded_path, runtime_dir, runtime_dir)?;
        self.write_installed_marker(component, runtime_dir)?;

        let _ = fs::remove_file(&sentinel);
        Ok(())
    }

    /// Install a PHP branch beside the current one and swap it in only once it is
    /// complete, so a locked file or a full disk keeps the working version. The
    /// swap also replaces the folder, so files of the previous patch cannot linger.
    pub(super) fn install_php_version(
        &self,
        downloaded_path: &Path,
        runtime_dir: &Path,
    ) -> Result<(), String> {
        let php_id = self.selected_php_id();
        if !is_safe_path_segment(&php_id) {
            return Err(format!("Invalid PHP version id: {:?}", php_id));
        }

        let versions_dir = crate::runtime::layout::php_versions_dir(runtime_dir);
        let install_dir = versions_dir.join(&php_id);
        let staging_dir = versions_dir.join(format!("{php_id}.new"));
        let backup_dir = versions_dir.join(format!("{php_id}.old"));

        // A crash between the two renames below leaves only the backup.
        if backup_dir.exists() {
            if install_dir.exists() {
                let _ = fs::remove_dir_all(&backup_dir);
            } else {
                let _ = fs::rename(&backup_dir, &install_dir);
            }
        }
        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir).map_err(|e| {
                format!(
                    "Failed to clear interrupted install at {}: {}",
                    staging_dir.display(),
                    e
                )
            })?;
        }

        let extracted = self
            .extract_component(
                BinaryComponent::Php,
                downloaded_path,
                &staging_dir,
                runtime_dir,
            )
            .and_then(|()| crate::runtime::locator::detect_php_binary_in_dir(&staging_dir))
            .and_then(|_| swap_in_directory(&staging_dir, &install_dir, &backup_dir));
        if let Err(error) = extracted {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(error);
        }

        let marked = self.write_installed_marker(BinaryComponent::Php, runtime_dir);
        let _ = fs::remove_dir_all(&backup_dir);
        marked
    }

    pub(super) fn normalize_phpmyadmin_install(&self, runtime_dir: &Path) -> Result<(), String> {
        let target_dir = runtime_dir.join("phpmyadmin");
        let source_dir = fs::read_dir(runtime_dir)
            .map_err(|e| format!("Failed to scan runtime directory: {}", e))?
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.is_dir()
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| {
                            name.eq_ignore_ascii_case("phpmyadmin")
                                || name.to_ascii_lowercase().starts_with("phpmyadmin-")
                        })
                        .unwrap_or(false)
                    && path.join("index.php").exists()
            })
            .ok_or_else(|| "phpMyAdmin archive did not contain an index.php".to_string())?;

        if source_dir == target_dir {
            return Ok(());
        }

        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)
                .map_err(|e| format!("Failed to replace phpMyAdmin runtime directory: {}", e))?;
        }

        fs::rename(&source_dir, &target_dir).map_err(|e| {
            format!(
                "Failed to move phpMyAdmin from {} to {}: {}",
                source_dir.display(),
                target_dir.display(),
                e
            )
        })
    }

    pub(super) fn write_installed_marker(
        &self,
        component: BinaryComponent,
        runtime_dir: &Path,
    ) -> Result<(), String> {
        let version = self.get_component_version(&component);
        let marker_content = format!(
            "version={}\ninstalled_at={:?}",
            version,
            std::time::SystemTime::now()
        );

        if component == BinaryComponent::Php {
            let php_id = self.selected_php_id();
            let versions_dir = crate::runtime::layout::php_versions_dir(runtime_dir);
            fs::create_dir_all(&versions_dir)
                .map_err(|e| format!("Failed to create PHP versions directory: {}", e))?;
            fs::write(
                crate::runtime::layout::marker_path(&versions_dir, &php_id),
                format!("id={}\n{}", php_id, marker_content),
            )
            .map_err(|e| format!("Failed to create PHP version marker file: {}", e))?;
        }

        let marker_file =
            crate::runtime::layout::marker_path(runtime_dir, self.marker_name(component));
        fs::write(&marker_file, marker_content)
            .map_err(|e| format!("Failed to create marker file: {}", e))
    }

    /// True when every component of the selected stack is installed. A failed
    /// first run can leave only some of them, and the setup wizard must come back
    /// in that case.
    pub fn is_installed(&self) -> bool {
        let Ok(runtime_dir) = self.get_runtime_dir() else {
            return false;
        };
        let selection = self
            .package_selection
            .clone()
            .unwrap_or_else(|| crate::config::AppSettings::load().package_selection);
        is_stack_installed(&runtime_dir, &selection)
    }

    /// Check which components are already installed with their versions
    pub fn get_installed_components(&self) -> std::collections::HashMap<String, String> {
        let mut installed = std::collections::HashMap::new();
        let runtime_dir = match self.get_runtime_dir() {
            Ok(dir) => dir,
            Err(_) => return installed,
        };

        for component in [
            "caddy",
            "php",
            "mysql",
            "postgresql",
            "adminer",
            "phpmyadmin",
        ] {
            if install_was_interrupted(&runtime_dir, component) {
                continue;
            }
            let marker_file = crate::runtime::layout::marker_path(&runtime_dir, component);
            if let Some(version) = read_marker_version(&marker_file) {
                installed.insert(component.to_string(), version);
            }
        }

        installed
    }
}

pub(super) fn install_sentinel_path(runtime_dir: &Path, marker_name: &str) -> PathBuf {
    runtime_dir.join(format!("{marker_name}.installing"))
}

/// An extraction of this component started and never finished, so neither its
/// marker (possibly from an older version) nor its binary can be trusted.
pub(super) fn install_was_interrupted(runtime_dir: &Path, marker_name: &str) -> bool {
    install_sentinel_path(runtime_dir, marker_name).exists()
}

/// Move a finished `staging` directory to `target`, putting back the previous
/// `target` if that fails. `backup` keeps the previous version until the caller
/// is done.
pub(super) fn swap_in_directory(
    staging: &Path,
    target: &Path,
    backup: &Path,
) -> Result<(), String> {
    let had_previous = target.exists();
    if had_previous {
        fs::rename(target, backup).map_err(|e| {
            format!(
                "Failed to replace {} (is PHP still running?): {}",
                target.display(),
                e
            )
        })?;
    }
    if let Err(error) = fs::rename(staging, target) {
        if had_previous {
            let _ = fs::rename(backup, target);
        }
        return Err(format!("Failed to install {}: {}", target.display(), error));
    }
    Ok(())
}

pub fn is_database_tool_installed(runtime_dir: &Path, tool_id: &str) -> bool {
    if crate::runtime::packages::is_adminer_id(tool_id) {
        !install_was_interrupted(runtime_dir, "adminer")
            && (runtime_dir.join("adminer").join("index.php").exists()
                || runtime_dir.join("adminer.php").exists())
    } else {
        !install_was_interrupted(runtime_dir, "phpmyadmin")
            && runtime_dir.join("phpmyadmin").join("index.php").exists()
    }
}

pub fn is_selected_database_installed(runtime_dir: &Path, postgresql: bool) -> bool {
    let (marker, folder, binary) = if postgresql {
        ("postgresql", "postgresql", "postgres")
    } else {
        ("mysql", "mysql", "mysqld")
    };

    if install_was_interrupted(runtime_dir, marker) {
        return false;
    }
    crate::runtime::layout::marker_path(runtime_dir, marker).exists()
        || [binary.to_string(), format!("{binary}.exe")]
            .iter()
            .any(|name| {
                runtime_dir.join(folder).join("bin").join(name).exists()
                    || runtime_dir.join("bin").join(name).exists()
            })
}

pub(super) fn is_caddy_installed(runtime_dir: &Path) -> bool {
    !install_was_interrupted(runtime_dir, "caddy")
        && (crate::runtime::layout::marker_path(runtime_dir, "caddy").exists()
            || ["caddy", "caddy.exe"].iter().any(|name| {
                runtime_dir.join(name).is_file() || runtime_dir.join("caddy").join(name).is_file()
            }))
}

pub(super) fn is_php_installed(runtime_dir: &Path, php_id: &str) -> bool {
    crate::runtime::layout::marker_path(runtime_dir, "php").exists()
        || crate::runtime::layout::php_version_marker(runtime_dir, php_id).exists()
        || runtime_dir.join("php").join("php.exe").exists()
}

pub(super) fn is_stack_installed(runtime_dir: &Path, selection: &PackageSelection) -> bool {
    is_caddy_installed(runtime_dir)
        && is_php_installed(runtime_dir, &selection.php)
        && is_selected_database_installed(runtime_dir, selection.is_postgresql())
        && is_database_tool_installed(runtime_dir, &selection.phpmyadmin)
}
