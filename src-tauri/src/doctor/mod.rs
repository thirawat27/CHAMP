//! Log Doctor: explains why a CHAMP service failed and how to fix it.
//!
//! Diagnosis runs fully offline against a bilingual knowledge base of known
//! service failures (`doctor-knowledge.json`). Exact error signatures always
//! work. When the optional Model2Vec model (`potion-base-8M`, ~7.6M parameters,
//! ~30 MB) is installed, semantic matching also recognises reworded or
//! unfamiliar log lines that describe the same problems.

mod model;

use crate::process::ServiceType;
use crate::runtime::locator::get_app_data_paths;
use download::{download_model, MODEL_FILES};
use knowledge::knowledge;
use model::StaticModel;
use rank::{candidate_lines, rank_issues, semantic_text, MAX_EXCERPT_LINES};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

mod download;
mod knowledge;
mod rank;

const MODEL_NAME: &str = "potion-base-8M";
const MODEL_PARAMETERS: &str = "7.6M";

struct ModelFile {
    name: &'static str,
    size: u64,
    sha256: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalizedText {
    pub en: String,
    pub th: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalizedSteps {
    pub en: Vec<String>,
    pub th: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct KnowledgeBase {
    issues: Vec<Issue>,
}

#[derive(Debug, Deserialize)]
struct Issue {
    id: String,
    services: Vec<String>,
    signatures: Vec<String>,
    examples: Vec<String>,
    title: LocalizedText,
    cause: LocalizedText,
    steps: LocalizedSteps,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

impl Issue {
    fn applies_to(&self, service: &str) -> bool {
        self.services
            .iter()
            .any(|item| item == "*" || item == service)
    }
}

#[derive(Debug, Serialize)]
pub struct DiagnosisMatch {
    pub id: String,
    pub title: LocalizedText,
    pub cause: LocalizedText,
    pub steps: LocalizedSteps,
    /// 0.0..=1.0
    pub confidence: f32,
    /// The log line that matched.
    pub evidence: String,
    /// "signature" for an exact known error, "ai" for a semantic match.
    pub method: &'static str,
    pub action: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Diagnosis {
    pub service: ServiceType,
    pub ai_used: bool,
    pub matches: Vec<DiagnosisMatch>,
    pub excerpt: Vec<String>,
    pub log_files: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LogDoctorStatus {
    pub installed: bool,
    pub downloading: bool,
    pub model_name: &'static str,
    pub parameters: &'static str,
    pub download_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
struct ModelDownloadProgress {
    downloaded_bytes: u64,
    total_bytes: u64,
    percent: u8,
}

struct LoadedModel {
    model: StaticModel,
    /// (issue index, normalized example embedding)
    examples: Vec<(usize, Vec<f32>)>,
}

static LOADED_MODEL: Mutex<Option<Arc<LoadedModel>>> = Mutex::new(None);
static DOWNLOADING: AtomicBool = AtomicBool::new(false);

pub(crate) fn service_log_files(service: ServiceType, logs_dir: &Path) -> Vec<PathBuf> {
    let names: &[&str] = match service {
        ServiceType::Caddy => &["caddy.log"],
        ServiceType::PhpFpm => &["php-fpm.log", "php-errors.log"],
        ServiceType::MySQL => &["mysql_init.log", "mysql.log"],
        ServiceType::PostgreSQL => &["postgresql_init.log", "postgresql.log"],
    };
    names.iter().map(|name| logs_dir.join(name)).collect()
}

pub(crate) fn diagnose(
    service: ServiceType,
    error_message: Option<&str>,
) -> Result<Diagnosis, String> {
    let paths = get_app_data_paths()?;
    let mut lines = Vec::new();
    let mut log_files = Vec::new();

    for path in service_log_files(service, &paths.logs_dir) {
        if let Some(file_lines) = crate::logs::read_tail_lines(&path) {
            log_files.push(path.to_string_lossy().to_string());
            lines.extend(file_lines);
        }
    }
    // The in-memory error is the freshest signal, so it goes last.
    if let Some(message) = error_message {
        lines.extend(message.lines().map(str::to_string));
    }

    let (candidates, has_errors) = candidate_lines(&lines);
    let model = loaded_model();
    // Semantic matching only looks at real error lines; healthy startup lines
    // ("ready for connections") share vocabulary with failures.
    let semantic_model = model.as_deref().filter(|_| has_errors);
    let matches = rank_issues(service.key(), &candidates, semantic_model);
    let excerpt_start = candidates.len().saturating_sub(MAX_EXCERPT_LINES);

    Ok(Diagnosis {
        service,
        ai_used: model.is_some(),
        matches,
        excerpt: candidates[excerpt_start..].to_vec(),
        log_files,
    })
}

fn model_dir() -> Result<PathBuf, String> {
    Ok(get_app_data_paths()?
        .base_dir
        .join("models")
        .join(MODEL_NAME))
}

/// Written when the user removes the model, so it is not downloaded again
/// automatically on the next start.
fn declined_marker() -> Result<PathBuf, String> {
    Ok(get_app_data_paths()?
        .base_dir
        .join("models")
        .join(format!("{MODEL_NAME}.declined")))
}

fn model_installed(dir: &Path) -> bool {
    MODEL_FILES.iter().all(|file| {
        fs::metadata(dir.join(file.name))
            .map(|metadata| metadata.len() == file.size)
            .unwrap_or(false)
    })
}

fn load_model(dir: &Path) -> Result<LoadedModel, String> {
    let model = StaticModel::load(dir)?;
    let examples = knowledge()
        .issues
        .iter()
        .enumerate()
        .flat_map(|(index, issue)| {
            issue
                .examples
                .iter()
                .chain(std::iter::once(&issue.title.en))
                .map(move |text| (index, text))
        })
        .filter_map(|(index, text)| Some((index, model.embed(&semantic_text(text))?)))
        .collect();
    Ok(LoadedModel { model, examples })
}

/// The cached model, loading it on first use when its files are installed.
fn loaded_model() -> Option<Arc<LoadedModel>> {
    let mut cache = LOADED_MODEL.lock().unwrap_or_else(|e| e.into_inner());
    if cache.is_none() {
        let dir = model_dir().ok()?;
        if !model_installed(&dir) {
            return None;
        }
        match load_model(&dir) {
            Ok(model) => *cache = Some(Arc::new(model)),
            Err(error) => {
                eprintln!("Log Doctor: failed to load {}: {}", MODEL_NAME, error);
                return None;
            }
        }
    }
    cache.clone()
}

fn unload_model() {
    *LOADED_MODEL.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn status() -> LogDoctorStatus {
    LogDoctorStatus {
        installed: model_dir()
            .map(|dir| model_installed(&dir))
            .unwrap_or(false),
        downloading: DOWNLOADING.load(Ordering::SeqCst),
        model_name: MODEL_NAME,
        parameters: MODEL_PARAMETERS,
        download_bytes: MODEL_FILES.iter().map(|file| file.size).sum(),
    }
}

/// Diagnose a service from its log files and its current error message.
#[tauri::command]
pub async fn diagnose_service(service: ServiceType, app: AppHandle) -> Result<Diagnosis, String> {
    let error_message = crate::process::runner::run_with_manager(&app, move |manager| {
        manager.update_health();
        manager
            .get_all_statuses()
            .get(&service)
            .and_then(|info| info.error_message.clone())
    })
    .await?;

    tauri::async_runtime::spawn_blocking(move || diagnose(service, error_message.as_deref()))
        .await
        .map_err(|e| format!("Log Doctor task failed: {}", e))?
}

#[tauri::command]
pub async fn get_log_doctor_status() -> LogDoctorStatus {
    status()
}

/// Download and verify the optional semantic model.
#[tauri::command]
pub async fn download_log_doctor_model(app: AppHandle) -> Result<LogDoctorStatus, String> {
    if DOWNLOADING.swap(true, Ordering::SeqCst) {
        return Err("The Log Doctor model is already downloading".to_string());
    }
    if let Ok(marker) = declined_marker() {
        let _ = fs::remove_file(marker);
    }
    let result = download_model(&app).await;
    DOWNLOADING.store(false, Ordering::SeqCst);
    // Other open views follow the status event, like the background download.
    let _ = app.emit("log-doctor-status", status());
    result.map(|_| status())
}

/// Install the model in the background on app start, unless it is already
/// installed or the user removed it. Emits `log-doctor-status` when done.
pub fn spawn_default_model_download(app: AppHandle) {
    let installed = model_dir().map(|dir| model_installed(&dir)).unwrap_or(true);
    let declined = declined_marker()
        .map(|marker| marker.exists())
        .unwrap_or(true);
    if installed || declined || DOWNLOADING.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Err(error) = download_model(&app).await {
            eprintln!("Log Doctor: automatic model download failed: {}", error);
        }
        DOWNLOADING.store(false, Ordering::SeqCst);
        let _ = app.emit("log-doctor-status", status());
    });
}

/// Delete the optional semantic model; signature matching keeps working.
#[tauri::command]
pub async fn remove_log_doctor_model() -> Result<LogDoctorStatus, String> {
    if DOWNLOADING.load(Ordering::SeqCst) {
        return Err("Wait for the model download to finish first".to_string());
    }
    unload_model();
    let dir = model_dir()?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Failed to remove model: {}", e))?;
    }
    let marker = declined_marker()?;
    if let Some(parent) = marker.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(marker, "");
    Ok(status())
}

#[cfg(test)]
mod tests;
