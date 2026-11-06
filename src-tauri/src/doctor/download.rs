//! Downloading the Log Doctor model.

use super::{model_dir, unload_model, ModelDownloadProgress, ModelFile};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub(super) const MODEL_REPO: &str = "minishlab/potion-base-8M";

/// Pinned revision so downloaded files always match the checksums below.
pub(super) const MODEL_REVISION: &str = "bf8b056651a2c21b8d2565580b8569da283cab23";

pub(super) const MODEL_FILES: [ModelFile; 2] = [
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

pub(super) const MODEL_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

pub(super) const MODEL_READ_TIMEOUT: Duration = Duration::from_secs(60);

pub(super) const MODEL_PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

pub(super) fn file_matches(path: &Path, expected: &ModelFile) -> bool {
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

pub(super) async fn download_model(app: &AppHandle) -> Result<(), String> {
    let dir = model_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create model folder: {}", e))?;

    let client = reqwest::Client::builder()
        .user_agent(concat!("CHAMP-Log-Doctor/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(MODEL_CONNECT_TIMEOUT)
        // A stalled connection would otherwise keep DOWNLOADING set forever,
        // blocking both a retry and removing the model.
        .read_timeout(MODEL_READ_TIMEOUT)
        .https_only(true)
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
            if last_emit.elapsed() >= MODEL_PROGRESS_INTERVAL {
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
