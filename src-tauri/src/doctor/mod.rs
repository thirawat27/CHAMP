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
use model::{cosine, StaticModel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const KNOWLEDGE_JSON: &str = include_str!("../../doctor-knowledge.json");

const MODEL_NAME: &str = "potion-base-8M";
const MODEL_REPO: &str = "minishlab/potion-base-8M";
/// Pinned revision so downloaded files always match the checksums below.
const MODEL_REVISION: &str = "bf8b056651a2c21b8d2565580b8569da283cab23";
const MODEL_PARAMETERS: &str = "7.6M";

struct ModelFile {
    name: &'static str,
    size: u64,
    sha256: &'static str,
}

const MODEL_FILES: [ModelFile; 2] = [
    ModelFile {
        name: "tokenizer.json",
        size: 683_666,
        sha256: "e67e803f624fb4d67dea1c730d06e1067e1b14d830e2c2202569e3ef0f70bb50",
    },
    ModelFile {
        name: "model.safetensors",
        size: 30_236_760,
        sha256: "f65d0f325faadc1e121c319e2faa41170d3fa07d8c89abd48ca5358d9a223de2",
    },
];

/// Bytes read from the end of each log file.
const LOG_TAIL_BYTES: u64 = 64 * 1024;
/// Error-looking lines kept for matching (most recent last).
const MAX_CANDIDATE_LINES: usize = 40;
/// Lines shown back to the user as evidence.
const MAX_EXCERPT_LINES: usize = 25;
const MAX_LINE_CHARS: usize = 400;
const MAX_MATCHES: usize = 3;
/// Minimum confidence for a match to be reported.
const MIN_CONFIDENCE: f32 = 0.6;
/// Cosine similarity at which a semantic match starts to count, and the
/// similarity treated as a certain match. Calibrated on potion-base-8M (see
/// `real_model_semantic_matching`): reworded lines describing the same failure
/// score ~0.62-0.78, neighbouring-but-wrong issues ~0.50-0.55.
const AI_SIMILARITY_FLOOR: f32 = 0.58;
const AI_SIMILARITY_CEILING: f32 = 0.8;
/// Semantic matches stay below exact signature matches.
const AI_MAX_CONFIDENCE: f32 = 0.85;

const ERROR_HINTS: [&str; 24] = [
    "error",
    "fail",
    "fatal",
    "warn",
    "denied",
    "cannot",
    "can't",
    "could not",
    "couldn't",
    "unable",
    "refused",
    "not found",
    "exception",
    "panic",
    "abort",
    "corrupt",
    "exited",
    "invalid",
    "missing",
    "no space",
    "in use",
    "not permitted",
    "killed",
    "exhausted",
];

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

fn knowledge() -> &'static KnowledgeBase {
    static KNOWLEDGE: OnceLock<KnowledgeBase> = OnceLock::new();
    KNOWLEDGE.get_or_init(|| {
        serde_json::from_str(KNOWLEDGE_JSON).expect("embedded doctor-knowledge.json must be valid")
    })
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

fn service_key(service: ServiceType) -> &'static str {
    match service {
        ServiceType::Caddy => "caddy",
        ServiceType::PhpFpm => "php-fpm",
        ServiceType::MySQL => "mysql",
        ServiceType::PostgreSQL => "postgresql",
    }
}

pub(crate) fn service_log_files(service: ServiceType, logs_dir: &Path) -> Vec<PathBuf> {
    let names: &[&str] = match service {
        ServiceType::Caddy => &["caddy.log"],
        ServiceType::PhpFpm => &["php-fpm.log", "php-errors.log"],
        ServiceType::MySQL => &["mysql_init.log", "mysql.log"],
        ServiceType::PostgreSQL => &["postgresql_init.log", "postgresql.log"],
    };
    names.iter().map(|name| logs_dir.join(name)).collect()
}

/// Read the last lines of a log file without loading the whole file.
pub(crate) fn read_tail_lines(path: &Path) -> Option<Vec<String>> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity((len - start) as usize);
    file.read_to_end(&mut bytes).ok()?;

    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if start > 0 && !lines.is_empty() {
        // The first line is probably cut in half.
        lines.remove(0);
    }
    Some(lines)
}

/// Keep recent, error-looking lines (deduplicated, most recent last).
///
/// The flag is false when no line looked like an error and the result is
/// just the log's last lines, shown for context.
fn candidate_lines(lines: &[String]) -> (Vec<String>, bool) {
    let clean = |line: &str| -> Option<String> {
        let trimmed = line.trim();
        (!trimmed.is_empty()).then(|| trimmed.chars().take(MAX_LINE_CHARS).collect())
    };

    let mut selected: Vec<String> = lines
        .iter()
        .filter(|line| {
            let lower = line.to_lowercase();
            ERROR_HINTS.iter().any(|hint| lower.contains(hint))
        })
        .filter_map(|line| clean(line))
        .collect();

    let has_errors = !selected.is_empty();
    if !has_errors {
        selected = lines.iter().filter_map(|line| clean(line)).collect();
        let keep_from = selected.len().saturating_sub(15);
        selected.drain(..keep_from);
    }

    // Deduplicate, keeping the most recent occurrence of each line.
    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<String> = selected
        .into_iter()
        .rev()
        .filter(|line| seen.insert(line.clone()))
        .collect();
    unique.truncate(MAX_CANDIDATE_LINES);
    unique.reverse();
    (unique, has_errors)
}

/// Words that appear in almost every candidate line (service names, log
/// levels) carry no meaning for matching and would pull every line toward the
/// same issues, so they are removed before embedding.
const SEMANTIC_STOPWORDS: [&str; 16] = [
    "mysql",
    "mysqld",
    "postgresql",
    "postgres",
    "php",
    "fpm",
    "cgi",
    "caddy",
    "champ",
    "innodb",
    "error",
    "fatal",
    "warning",
    "note",
    "info",
    "level",
];

/// Prepare text for embedding: drop paths/URLs, numbers, timestamps, and stopwords.
fn semantic_text(text: &str) -> String {
    text.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | '='
            )
    })
    .filter(|word| !word.contains('/') && !word.contains('\\'))
    .flat_map(|word| word.split(['-', '_', '.', ':']))
    .map(|word| {
        word.trim_matches(|ch: char| !ch.is_alphanumeric())
            .to_lowercase()
    })
    .filter(|word| {
        word.chars().any(char::is_alphabetic) && !SEMANTIC_STOPWORDS.contains(&word.as_str())
    })
    .collect::<Vec<_>>()
    .join(" ")
}

fn signature_confidence(signature: &str) -> f32 {
    // Longer signatures are more specific, so they win ties against short,
    // generic ones (e.g. "listen tcp :80: bind: permission denied" beats
    // "permission denied").
    0.86 + (signature.len() as f32 / 400.0).min(0.12)
}

fn ai_confidence(similarity: f32) -> f32 {
    let scaled = (similarity - AI_SIMILARITY_FLOOR) / (AI_SIMILARITY_CEILING - AI_SIMILARITY_FLOOR);
    (MIN_CONFIDENCE + scaled * (AI_MAX_CONFIDENCE - MIN_CONFIDENCE)).min(AI_MAX_CONFIDENCE)
}

/// Rank knowledge-base issues against candidate log lines (most recent last).
fn rank_issues(
    service: &str,
    lines: &[String],
    model: Option<&LoadedModel>,
) -> Vec<DiagnosisMatch> {
    let issues = &knowledge().issues;
    // (confidence, evidence, method) per issue
    let mut best: Vec<Option<(f32, String, &'static str)>> = vec![None; issues.len()];
    let mut consider = |index: usize, confidence: f32, evidence: &str, method: &'static str| {
        let replace = match &best[index] {
            Some((current, _, _)) => confidence > *current,
            None => true,
        };
        if replace {
            best[index] = Some((confidence, evidence.to_string(), method));
        }
    };

    for line in lines.iter().rev() {
        let lower = line.to_lowercase();
        for (index, issue) in issues.iter().enumerate() {
            if !issue.applies_to(service) {
                continue;
            }
            if let Some(signature) = issue
                .signatures
                .iter()
                .filter(|signature| lower.contains(signature.as_str()))
                .max_by_key(|signature| signature.len())
            {
                consider(index, signature_confidence(signature), line, "signature");
            }
        }
    }

    if let Some(loaded) = model {
        // A static embedding model separates "related" from "unrelated" well,
        // but neighbouring issues of one service score close together. Only the
        // single most similar issue is proposed; signatures can add more.
        let mut top: Option<(f32, usize, &String)> = None;
        for line in lines.iter().rev() {
            let Some(vector) = loaded.model.embed(&semantic_text(line)) else {
                continue;
            };
            for (index, example) in &loaded.examples {
                if !issues[*index].applies_to(service) {
                    continue;
                }
                let similarity = cosine(&vector, example);
                if top.is_none_or(|(current, _, _)| similarity > current) {
                    top = Some((similarity, *index, line));
                }
            }
        }
        if let Some((similarity, index, line)) = top {
            if similarity >= AI_SIMILARITY_FLOOR {
                consider(index, ai_confidence(similarity), line, "ai");
            }
        }
    }

    let mut matches: Vec<DiagnosisMatch> = best
        .into_iter()
        .enumerate()
        .filter_map(|(index, scored)| {
            let (confidence, evidence, method) = scored?;
            if confidence < MIN_CONFIDENCE {
                return None;
            }
            let issue = &issues[index];
            Some(DiagnosisMatch {
                id: issue.id.clone(),
                title: issue.title.clone(),
                cause: issue.cause.clone(),
                steps: issue.steps.clone(),
                confidence: (confidence * 100.0).round() / 100.0,
                evidence,
                method,
                action: issue.action.clone(),
                url: issue.url.clone(),
            })
        })
        .collect();

    matches.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    matches.truncate(MAX_MATCHES);
    matches
}

pub(crate) fn diagnose(
    service: ServiceType,
    error_message: Option<&str>,
) -> Result<Diagnosis, String> {
    let paths = get_app_data_paths()?;
    let mut lines = Vec::new();
    let mut log_files = Vec::new();

    for path in service_log_files(service, &paths.logs_dir) {
        if let Some(file_lines) = read_tail_lines(&path) {
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
    let matches = rank_issues(service_key(service), &candidates, semantic_model);
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

fn file_matches(path: &Path, expected: &ModelFile) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    if file.metadata().map(|m| m.len()).unwrap_or(0) != expected.size {
        return false;
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => hasher.update(&buffer[..read]),
            Err(_) => return false,
        }
    }
    hex::encode(hasher.finalize()) == expected.sha256
}

async fn download_model(app: &AppHandle) -> Result<(), String> {
    let dir = model_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create model folder: {}", e))?;

    let client = reqwest::Client::builder()
        .user_agent(concat!("CHAMP-Log-Doctor/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let total_bytes: u64 = MODEL_FILES.iter().map(|file| file.size).sum();
    let mut completed_bytes = 0_u64;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let emit = |downloaded: u64| {
        let _ = app.emit(
            "log-doctor-progress",
            ModelDownloadProgress {
                downloaded_bytes: downloaded,
                total_bytes,
                percent: ((downloaded as f64 / total_bytes as f64) * 100.0).round() as u8,
            },
        );
    };

    for expected in &MODEL_FILES {
        let target = dir.join(expected.name);
        if file_matches(&target, expected) {
            completed_bytes += expected.size;
            continue;
        }

        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            MODEL_REPO, MODEL_REVISION, expected.name
        );
        let mut response = client
            .get(&url)
            .send()
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|e| format!("Failed to download {}: {}", expected.name, e))?;

        let partial = dir.join(format!("{}.part", expected.name));
        let mut file = std::io::BufWriter::new(
            File::create(&partial).map_err(|e| format!("Failed to create file: {}", e))?,
        );
        let mut hasher = Sha256::new();
        let mut file_bytes = 0_u64;

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Failed while downloading {}: {}", expected.name, e))?
        {
            file_bytes += chunk.len() as u64;
            if file_bytes > expected.size {
                break;
            }
            hasher.update(&chunk);
            file.write_all(&chunk)
                .map_err(|e| format!("Failed to write {}: {}", expected.name, e))?;
            if last_emit.elapsed() >= Duration::from_millis(250) {
                last_emit = Instant::now();
                emit(completed_bytes + file_bytes);
            }
        }
        file.flush()
            .map_err(|e| format!("Failed to write {}: {}", expected.name, e))?;
        drop(file);

        let checksum = hex::encode(hasher.finalize());
        if file_bytes != expected.size || checksum != expected.sha256 {
            let _ = fs::remove_file(&partial);
            return Err(format!(
                "{} failed verification (size {} bytes, SHA-256 {}). Please try again.",
                expected.name, file_bytes, checksum
            ));
        }

        fs::rename(&partial, &target)
            .map_err(|e| format!("Failed to save {}: {}", expected.name, e))?;
        completed_bytes += expected.size;
    }

    emit(total_bytes);
    unload_model();
    Ok(())
}

/// Diagnose a service from its log files and its current error message.
#[tauri::command]
pub async fn diagnose_service(service: ServiceType, app: AppHandle) -> Result<Diagnosis, String> {
    let error_message = crate::commands::run_with_manager(&app, move |manager| {
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
mod tests {
    use super::*;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn knowledge_base_is_complete() {
        let issues = &knowledge().issues;
        assert!(issues.len() >= 20);

        let mut ids = std::collections::HashSet::new();
        let known_services = ["*", "caddy", "php-fpm", "mysql", "postgresql"];
        let known_actions = [
            "open_settings",
            "open_logs",
            "open_url",
            "start_php",
            "open_mysql_data",
            "open_postgresql_data",
            "open_runtime",
        ];
        for issue in issues {
            assert!(ids.insert(issue.id.clone()), "duplicate id {}", issue.id);
            assert!(
                !issue.signatures.is_empty(),
                "{} has no signatures",
                issue.id
            );
            assert!(!issue.examples.is_empty(), "{} has no examples", issue.id);
            assert!(issue
                .services
                .iter()
                .all(|s| known_services.contains(&s.as_str())));
            assert!(
                issue.signatures.iter().all(|s| *s == s.to_lowercase()),
                "{}",
                issue.id
            );
            assert!(!issue.title.en.is_empty() && !issue.title.th.is_empty());
            assert!(!issue.cause.en.is_empty() && !issue.cause.th.is_empty());
            assert_eq!(issue.steps.en.len(), issue.steps.th.len(), "{}", issue.id);
            if let Some(action) = &issue.action {
                assert!(known_actions.contains(&action.as_str()), "{}", issue.id);
                assert_eq!(action == "open_url", issue.url.is_some(), "{}", issue.id);
            }
        }
    }

    #[test]
    fn every_example_is_caught_by_its_own_signatures_or_model() {
        // Examples double as regression tests for the signature list: each
        // issue must recognise at least one of its own examples without AI.
        for issue in &knowledge().issues {
            let service = issue
                .services
                .iter()
                .find(|s| s.as_str() != "*")
                .map(String::as_str)
                .unwrap_or("caddy");
            let matches = rank_issues(
                service,
                &lines(
                    &issue
                        .examples
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                ),
                None,
            );
            assert!(
                matches.iter().any(|m| m.id == issue.id),
                "{} did not match its own examples",
                issue.id
            );
        }
    }

    #[test]
    fn port_conflict_is_detected_from_signature() {
        let matches = rank_issues(
            "caddy",
            &lines(&[
                "Error: loading initial config: listen tcp :8080: bind: address already in use",
            ]),
            None,
        );
        assert_eq!(matches[0].id, "port-in-use");
        assert_eq!(matches[0].method, "signature");
    }

    #[test]
    fn specific_signature_outranks_generic_one() {
        let matches = rank_issues(
            "caddy",
            &lines(&["listen tcp :80: bind: permission denied"]),
            None,
        );
        assert_eq!(matches[0].id, "low-port-permission");
        assert!(matches.iter().any(|m| m.id == "permission-denied"));
    }

    #[test]
    fn service_filter_hides_unrelated_issues() {
        let matches = rank_issues(
            "caddy",
            &lines(&["InnoDB: Unable to lock ./ibdata1 error: 11"]),
            None,
        );
        assert!(matches.iter().all(|m| m.id != "mysql-locked"));
    }

    #[test]
    fn locked_ibdata_outranks_dd_storage_engine_failure() {
        // Windows mysqld reports a data folder held by another mysqld as
        // "must be writable", followed by the generic DD engine failure.
        let matches = rank_issues(
            "mysql",
            &lines(&[
                "[ERROR] [MY-012271] [InnoDB] The innodb_system data file 'ibdata1' must be writable",
                "[ERROR] [MY-010334] [Server] Failed to initialize DD Storage Engine",
                "[ERROR] [MY-010020] [Server] Data Dictionary initialization failed.",
            ]),
            None,
        );
        assert_eq!(matches[0].id, "mysql-locked");
    }

    #[test]
    fn unknown_errors_return_no_matches() {
        let matches = rank_issues("caddy", &lines(&["everything is fine"]), None);
        assert!(matches.is_empty());
    }

    #[test]
    fn candidate_lines_prefer_errors_and_dedupe() {
        let input = lines(&[
            "info: starting",
            "ERROR: port busy",
            "info: retry",
            "ERROR: port busy",
            "fatal: giving up",
        ]);
        assert_eq!(
            candidate_lines(&input),
            (lines(&["ERROR: port busy", "fatal: giving up"]), true)
        );

        let calm = lines(&["one", "two"]);
        assert_eq!(candidate_lines(&calm), (calm, false));
    }

    #[test]
    fn tail_reader_drops_partial_first_line() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("big.log");
        let mut content = "x".repeat(LOG_TAIL_BYTES as usize);
        content.push_str("\nlast line\n");
        fs::write(&path, content).unwrap();

        assert_eq!(read_tail_lines(&path).unwrap(), lines(&["last line"]));
    }

    /// Manual calibration check against the real model:
    /// `CHAMP_DOCTOR_MODEL_DIR=<dir> cargo test doctor -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_model_semantic_matching() {
        let Some(dir) = std::env::var_os("CHAMP_DOCTOR_MODEL_DIR") else {
            return;
        };
        let loaded = load_model(Path::new(&dir)).expect("model should load");
        let probes = [
            ("mysql", "Another program has already taken TCP port 3306"),
            (
                "php-fpm",
                "php-cgi.exe - System Error: VCRUNTIME140.dll missing",
            ),
            (
                "postgresql",
                "server files are from an older PostgreSQL release",
            ),
            ("mysql", "the hard drive has no free capacity remaining"),
            (
                "caddy",
                "upstream php backend unreachable, connection was refused",
            ),
            (
                "postgresql",
                "another server instance holds the lock on the data directory",
            ),
            ("caddy", "served 200 GET /index.php in 3ms"),
            (
                "postgresql",
                "database system is ready to accept connections",
            ),
            (
                "mysql",
                "ready for connections. Version: '8.4.3'  port: 3306",
            ),
        ];
        for (service, probe) in probes {
            let Some(vector) = loaded.model.embed(&semantic_text(probe)) else {
                continue;
            };
            let mut scores: Vec<(f32, &str)> = loaded
                .examples
                .iter()
                .map(|(index, example)| {
                    (
                        cosine(&vector, example),
                        knowledge().issues[*index].id.as_str(),
                    )
                })
                .collect();
            scores.sort_by(|a, b| b.0.total_cmp(&a.0));
            println!("{probe}\n  -> {:?}", &scores[..3]);
            let ranked = rank_issues(service, &lines(&[probe]), Some(&loaded));
            println!(
                "  ranked: {:?}",
                ranked
                    .iter()
                    .map(|m| (m.id.as_str(), m.confidence, m.method))
                    .collect::<Vec<_>>()
            );
        }
    }
}
