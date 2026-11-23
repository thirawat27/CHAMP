//! Project background commands: long-running PHP processes such as
//! `artisan queue:work`, `bin/console messenger:consume`, or `spark tasks:work`.
//!
//! Only PHP commands are allowed (CHAMP dropped other runtimes in 1.4.0):
//! CHAMP's PHP CLI runs a script inside the project folder with CHAMP's
//! php.ini, so database, mail, and add-on settings match the website. Users
//! define workers in the UI; agents may start and stop them but never create
//! or edit one. They reuse the add-on process runtime (no ports to wait for)
//! and stop with the stack.

use super::runtime::{self, LaunchPlan};
use super::spec::StopMethod;
use crate::config::{AppSettings, ProjectWorker};
use crate::process::ServiceState;
use crate::runtime::locator::get_app_data_paths;
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

const MAX_ARGS: usize = 32;
const MAX_ARG_LEN: usize = 512;

/// Runtime key, kept apart from add-on ids.
fn runtime_key(id: &str) -> String {
    format!("worker:{id}")
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerStatus {
    #[serde(flatten)]
    pub worker: ProjectWorker,
    pub state: ServiceState,
    pub error: Option<String>,
    /// `php artisan queue:work`, for display.
    pub command_line: String,
    pub log_path: Option<PathBuf>,
}

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && bytes[bytes.len() - 1] != b'-'
}

/// A relative path that stays inside the project (`artisan`, `bin/console`).
fn script_inside(project_dir: &Path, script: &str) -> Option<PathBuf> {
    let relative = Path::new(script);
    if script.is_empty()
        || script.starts_with('-')
        || !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return None;
    }
    let path = project_dir.join(relative);
    path.is_file().then_some(path)
}

pub(super) fn validate(worker: &ProjectWorker, projects_dir: &Path) -> Result<PathBuf, String> {
    if !valid_id(&worker.id) {
        return Err(format!(
            "Invalid id `{}`. Use up to 32 lowercase letters, digits, or hyphens, starting with a letter (for example `queue`).",
            worker.id
        ));
    }
    let name = worker.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("Give the background task a name of 1-40 characters.".to_string());
    }
    let project_dir = crate::projects::project_dir(projects_dir, &worker.project)?;
    if worker.args.is_empty() || worker.args.len() > MAX_ARGS {
        return Err(format!(
            "Give the PHP script and its arguments (at most {MAX_ARGS}), for example: artisan queue:work"
        ));
    }
    if worker
        .args
        .iter()
        .any(|arg| arg.len() > MAX_ARG_LEN || arg.contains('\0'))
    {
        return Err(format!(
            "Arguments must be shorter than {MAX_ARG_LEN} characters."
        ));
    }
    if script_inside(&project_dir, &worker.args[0]).is_none() {
        return Err(format!(
            "`{}` is not a script inside {}. The first argument must be a PHP file in the project, such as artisan or bin/console.",
            worker.args[0], worker.project
        ));
    }
    Ok(project_dir)
}

fn php_cli() -> Result<(PathBuf, Option<PathBuf>), String> {
    let paths = crate::runtime::locator::locate_runtime_binaries()?;
    let php = crate::runtime::locator::sibling_binary(&paths.php_cgi, "php");
    if !php.is_file() {
        return Err(format!(
            "CHAMP's PHP command line was not found at {}. Reinstall PHP in Settings.",
            php.display()
        ));
    }
    Ok((php, paths.php_ini.is_file().then_some(paths.php_ini)))
}

fn command_line(worker: &ProjectWorker) -> String {
    std::iter::once("php".to_string())
        .chain(worker.args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn log_path(logs_dir: &Path, id: &str) -> PathBuf {
    logs_dir.join(format!("worker-{id}.log"))
}

pub fn statuses() -> Result<Vec<WorkerStatus>, String> {
    let paths = get_app_data_paths()?;
    Ok(AppSettings::load()
        .workers
        .into_iter()
        .map(|worker| {
            let (state, error, _) = runtime::snapshot(&runtime_key(&worker.id));
            let log = log_path(&paths.logs_dir, &worker.id);
            WorkerStatus {
                command_line: command_line(&worker),
                state,
                error,
                log_path: log.exists().then_some(log),
                worker,
            }
        })
        .collect())
}

pub fn start_blocking(id: &str) -> Result<(), String> {
    let paths = get_app_data_paths()?;
    let key = runtime_key(id);
    let worker = AppSettings::load()
        .workers
        .into_iter()
        .find(|worker| worker.id == id)
        .ok_or_else(|| format!("Unknown background task `{id}`."))?;
    let plan = (|| {
        let project_dir = validate(&worker, &paths.projects_dir)?;
        let (php, php_ini) = php_cli()?;
        let mut args: Vec<OsString> = Vec::new();
        if let Some(ini) = php_ini {
            args.push("-c".into());
            args.push(ini.into_os_string());
        }
        args.extend(worker.args.iter().map(OsString::from));
        std::fs::create_dir_all(&paths.logs_dir)
            .map_err(|e| format!("Failed to create logs folder: {e}"))?;
        Ok::<_, String>(LaunchPlan {
            name: worker.name.clone(),
            binary: php,
            args,
            data_dir: project_dir,
            log_path: log_path(&paths.logs_dir, id),
            ports: Vec::new(),
            stop: StopMethod::Terminate,
            owned_binary: false,
            env: Vec::new(),
        })
    })();
    match plan {
        Ok(plan) => runtime::start(&key, plan),
        Err(message) => {
            runtime::set_error(&key, &message);
            Err(message)
        }
    }
}

pub fn stop_blocking(id: &str) -> Result<(), String> {
    runtime::stop(&runtime_key(id))
}

/// Start the workers marked to start with the stack.
pub(super) fn start_enabled_blocking() -> Vec<String> {
    AppSettings::load()
        .workers
        .into_iter()
        .filter(|worker| worker.enabled)
        .filter_map(|worker| start_blocking(&worker.id).err())
        .collect()
}

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| format!("Background task failed: {e}"))?
}

#[tauri::command]
pub async fn get_workers() -> Result<Vec<WorkerStatus>, String> {
    blocking(statuses).await
}

/// Add or replace a worker (matched by id). A running copy restarts.
#[tauri::command]
pub async fn save_worker(worker: ProjectWorker) -> Result<Vec<WorkerStatus>, String> {
    let projects_dir = get_app_data_paths()?.projects_dir;
    let mut worker = worker;
    worker.name = worker.name.trim().to_string();
    validate(&worker, &projects_dir)?;
    let id = worker.id.clone();
    AppSettings::update(|settings| {
        match settings
            .workers
            .iter_mut()
            .find(|saved| saved.id == worker.id)
        {
            Some(saved) => *saved = worker,
            None => settings.workers.push(worker),
        }
    })?;
    let (state, _, _) = runtime::snapshot(&runtime_key(&id));
    if state.is_running() {
        let _ = blocking(move || {
            stop_blocking(&id)?;
            start_blocking(&id)
        })
        .await;
    }
    get_workers().await
}

#[tauri::command]
pub async fn remove_worker(id: String) -> Result<Vec<WorkerStatus>, String> {
    let stop_id = id.clone();
    blocking(move || stop_blocking(&stop_id)).await?;
    runtime::forget(&runtime_key(&id));
    AppSettings::update(|settings| settings.workers.retain(|worker| worker.id != id))?;
    get_workers().await
}

#[tauri::command]
pub async fn start_worker(id: String) -> Result<Vec<WorkerStatus>, String> {
    blocking(move || start_blocking(&id)).await?;
    get_workers().await
}

#[tauri::command]
pub async fn stop_worker(id: String) -> Result<Vec<WorkerStatus>, String> {
    blocking(move || stop_blocking(&id)).await?;
    get_workers().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn worker(args: &[&str]) -> ProjectWorker {
        ProjectWorker {
            id: "queue".to_string(),
            project: "shop".to_string(),
            name: "Queue".to_string(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            enabled: false,
        }
    }

    #[test]
    fn workers_must_run_a_script_inside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("shop");
        fs::create_dir_all(project.join("bin")).unwrap();
        fs::write(project.join("artisan"), "<?php").unwrap();
        fs::write(project.join("bin/console"), "<?php").unwrap();
        fs::write(dir.path().join("outside.php"), "<?php").unwrap();

        assert!(validate(&worker(&["artisan", "queue:work"]), dir.path()).is_ok());
        assert!(validate(&worker(&["bin/console", "messenger:consume"]), dir.path()).is_ok());
        for args in [
            &[][..],
            &["-r", "echo 1;"][..],
            &["../outside.php"][..],
            &["missing.php"][..],
            &["/etc/passwd"][..],
        ] {
            assert!(validate(&worker(args), dir.path()).is_err(), "{args:?}");
        }
        let mut bad_project = worker(&["artisan"]);
        bad_project.project = "../shop".to_string();
        assert!(validate(&bad_project, dir.path()).is_err());
        let mut bad_id = worker(&["artisan"]);
        bad_id.id = "Queue".to_string();
        assert!(validate(&bad_id, dir.path()).is_err());
    }

    #[test]
    fn runtime_keys_never_collide_with_addons() {
        assert_eq!(runtime_key("redis"), "worker:redis");
        assert_eq!(
            command_line(&worker(&["artisan", "queue:work"])),
            "php artisan queue:work"
        );
    }
}
