//! Downloading archives, in parallel ranges when the host allows it.

use super::verify::cached_archive_is_valid;
use super::{
    BinaryComponent, DownloadFinalizeRequest, DownloadMetadata, DownloadProgressMeta,
    DownloadedFile, ProgressCallback, RuntimeDownloader, SegmentDownloadResult, MERGE_BUFFER_BYTES,
};
use crate::runtime::packages::is_safe_path_segment;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use reqwest::{RequestBuilder, StatusCode};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

impl RuntimeDownloader {
    /// Default User-Agent for runtime downloads. CHAMP identifies itself
    /// honestly rather than impersonating a browser. Hosts that genuinely
    /// require a browser-like UA get one via the per-request exception in
    /// `build_download_request` / `build_head_request`. (M-11)
    pub(super) fn user_agent(&self) -> &'static str {
        concat!(
            "CHAMP-Runtime-Installer/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/thirawat27/CHAMP)"
        )
    }

    /// Browser-like User-Agent kept only for hosts that reject non-browser
    /// clients. Currently used exclusively for dev.mysql.com. (M-11)
    const MYSQL_BROWSER_USER_AGENT: &'static str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

    /// MySQL downloads via dev.mysql.com reject non-browser clients, so this one
    /// host keeps a browser-like UA alongside its Referer/Accept headers. Every
    /// other host gets the honest CHAMP UA. (M-11)
    pub(super) fn with_host_headers(
        &self,
        request: RequestBuilder,
        url: &str,
        component: BinaryComponent,
    ) -> RequestBuilder {
        if component == BinaryComponent::MySQL && url.contains("dev.mysql.com") {
            return with_mysql_site_headers(
                request.header("User-Agent", Self::MYSQL_BROWSER_USER_AGENT),
            );
        }

        request.header("User-Agent", self.user_agent())
    }

    pub(super) fn build_download_request(
        &self,
        url: &str,
        component: BinaryComponent,
    ) -> RequestBuilder {
        self.with_host_headers(self.client.get(url), url, component)
    }

    pub(super) fn build_head_request(
        &self,
        url: &str,
        component: BinaryComponent,
    ) -> RequestBuilder {
        self.with_host_headers(self.client.head(url), url, component)
    }

    /// Extract file extension from URL
    pub(super) fn get_extension_from_url(url: &str) -> String {
        let url = url.split(['?', '#']).next().unwrap_or(url);
        // Get the filename from the URL
        if let Some(filename) = url.split('/').next_back() {
            // Check for .tar.gz first (compound extension)
            if filename.ends_with(".tar.gz") {
                return "tar.gz".to_string();
            }
            // Otherwise get the extension after the last dot
            if let Some(ext) = filename.split('.').next_back() {
                return ext.to_string();
            }
        }
        // Default to zip if we can't determine
        "zip".to_string()
    }

    pub(super) fn is_supported_download_extension(extension: &str) -> bool {
        matches!(extension, "zip" | "gz" | "tar.gz" | "xz" | "tar.xz" | "php")
    }

    /// Download a single binary component
    pub(super) async fn download_component(
        &self,
        component: BinaryComponent,
        dest_dir: &Path,
        progress_cb: &ProgressCallback,
        current: u8,
        total: u8,
    ) -> Result<PathBuf, String> {
        let url = self
            .resolve_moved_url(self.get_binary_url(component), component)
            .await;
        if url.trim().is_empty() {
            return Err(format!(
                "{} does not have an official portable binary URL for {:?}. Use the Windows official runtime or configure a trusted platform package source.",
                self.component_display_name(component),
                self.platform
            ));
        }
        let extension = Self::get_extension_from_url(&url);
        if !Self::is_supported_download_extension(&extension) {
            return Err(format!(
                "{} uses an unsupported archive format '.{}'. CHAMP can install .zip, .tar.gz, .tar.xz, and .php runtime packages.",
                self.component_display_name(component),
                extension
            ));
        }
        let version = self.get_component_version(&component);
        if !is_safe_path_segment(&version) {
            return Err(format!(
                "{} has an invalid version in the runtime catalog: {:?}",
                self.component_display_name(component),
                version
            ));
        }
        let file_path = dest_dir.join(format!(
            "{}-{}.{}",
            component.binary_name(),
            version,
            extension
        ));

        if let Some(size) = self
            .reusable_cached_archive(component, &file_path, &extension)
            .await
        {
            let meta = DownloadProgressMeta {
                progress_cb,
                component_name: self.component_name(component),
                component_display: self.component_display_name(component),
                current,
                total,
                version: &version,
            };
            meta.emit(100, size, size);
            return Ok(file_path);
        }

        eprintln!("[DEBUG] Platform: {:?}", self.platform);
        eprintln!("[DEBUG] Downloading {} from: {}", component.name(), url);
        eprintln!("[DEBUG] Full URL ({} chars): {}", url.len(), url);

        let probed_metadata = self.probe_download_metadata(&url, component).await;

        // Create parent directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let partial_path = file_path.with_file_name(format!(
            "{}.part",
            file_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("download")
        ));
        let _ = fs::remove_file(&partial_path);
        let progress_meta = DownloadProgressMeta {
            progress_cb,
            component_name: self.component_name(component),
            component_display: self.component_display_name(component),
            current,
            total,
            version: &version,
        };

        let finalize_request = DownloadFinalizeRequest {
            component,
            extension: &extension,
            partial_path: &partial_path,
            file_path: &file_path,
            progress_meta: &progress_meta,
        };

        let mut segmented_attempted = false;
        if let Some(metadata) = &probed_metadata {
            if metadata.accepts_ranges && metadata.total_bytes >= SEGMENTED_DOWNLOAD_MIN_BYTES {
                segmented_attempted = true;
                if let Some(result) = self
                    .try_segmented_download(&url, metadata.total_bytes, finalize_request)
                    .await
                {
                    return result;
                }
            }
        }

        let mut response = self
            .build_download_request(&url, component)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch {}: {}", component.name(), e))?;

        // Check status code
        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "HTTP error {}: Failed to download {}\nURL: {}",
                status.as_u16(),
                component.name(),
                url
            ));
        }

        // Get final URL after redirects
        let final_url = response.url().clone();
        if final_url.as_str() != url {
            eprintln!("Redirected: {} -> {}", url, final_url);
        }

        // Check content type
        if let Some(content_type) = response.headers().get("content-type") {
            if let Ok(ct) = content_type.to_str() {
                if ct.contains("text/html") {
                    return Err(format!(
                        "Server returned HTML instead of binary. URL may be incorrect: {}",
                        final_url
                    ));
                }
            }
        }

        let total_bytes = response
            .content_length()
            .or(probed_metadata
                .as_ref()
                .map(|metadata| metadata.total_bytes))
            .unwrap_or(0);
        let accepts_ranges = response
            .headers()
            .get(ACCEPT_RANGES)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.eq_ignore_ascii_case("bytes"))
            .unwrap_or(false)
            || probed_metadata
                .as_ref()
                .map(|metadata| metadata.accepts_ranges)
                .unwrap_or(false);

        let _ = fs::remove_file(&partial_path);

        if !segmented_attempted
            && self
                .should_use_segmented_download(total_bytes, accepts_ranges, &url, component)
                .await
        {
            drop(response);
            if let Some(result) = self
                .try_segmented_download(&url, total_bytes, finalize_request)
                .await
            {
                return result;
            }
            response = self
                .build_download_request(&url, component)
                .send()
                .await
                .map_err(|e| format!("Failed to fetch {}: {}", component.name(), e))?;
        }

        let mut file = BufWriter::with_capacity(
            MERGE_BUFFER_BYTES,
            File::create(&partial_path).map_err(|e| format!("Failed to create file: {}", e))?,
        );
        let mut hasher = Sha256::new();
        let mut magic_bytes = Vec::with_capacity(4);
        let mut downloaded_bytes = 0_u64;
        let mut last_percent = u8::MAX;
        let mut last_emit = Instant::now() - Duration::from_secs(1);

        progress_meta.emit(0, downloaded_bytes, total_bytes);

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Failed while downloading {}: {}", component.name(), e))?
        {
            if magic_bytes.len() < 4 {
                let remaining = 4 - magic_bytes.len();
                magic_bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
            }

            hasher.update(&chunk);
            file.write_all(&chunk)
                .map_err(|e| format!("Failed to write to file: {}", e))?;
            downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as u64);

            let percent = Self::download_percent(downloaded_bytes, total_bytes);

            if percent != last_percent || last_emit.elapsed() >= PROGRESS_EMIT_INTERVAL {
                last_percent = percent;
                last_emit = Instant::now();
                progress_meta.emit(percent, downloaded_bytes, total_bytes);
            }
        }

        file.flush()
            .map_err(|e| format!("Failed to flush downloaded file: {}", e))?;
        drop(file);

        self.finalize_downloaded_file(
            finalize_request,
            DownloadedFile {
                downloaded_bytes,
                total_bytes,
                magic_bytes,
                hasher,
            },
        )
    }

    /// Download in parallel ranges. `None` when that failed and the caller should
    /// fall back to a single stream.
    pub(super) async fn try_segmented_download(
        &self,
        url: &str,
        total_bytes: u64,
        request: DownloadFinalizeRequest<'_>,
    ) -> Option<Result<PathBuf, String>> {
        match self
            .download_component_segmented(
                url,
                request.component,
                request.partial_path,
                total_bytes,
                request.progress_meta,
            )
            .await
        {
            Ok(downloaded_file) => Some(self.finalize_downloaded_file(request, downloaded_file)),
            Err(error) => {
                eprintln!(
                    "Segmented download failed for {}, falling back to single stream: {}",
                    request.component.name(),
                    error
                );
                let _ = fs::remove_file(request.partial_path);
                None
            }
        }
    }

    /// Size of an archive left by an earlier attempt, when it still passes the
    /// checks a fresh download must pass. A bad one is deleted so it cannot fail
    /// every retry.
    pub(super) async fn reusable_cached_archive(
        &self,
        component: BinaryComponent,
        file_path: &Path,
        extension: &str,
    ) -> Option<u64> {
        let size = fs::metadata(file_path).ok()?.len();
        let expected_checksum = self.get_expected_checksum(&component);

        let archive = file_path.to_path_buf();
        let archive_extension = extension.to_string();
        let valid = tokio::task::spawn_blocking(move || {
            cached_archive_is_valid(&archive, &archive_extension, expected_checksum.as_deref())
        })
        .await
        .unwrap_or(false);

        if valid {
            Some(size)
        } else {
            eprintln!("Discarding unusable cached archive {}", file_path.display());
            let _ = fs::remove_file(file_path);
            None
        }
    }

    /// windows.php.net and cdn.mysql.com move superseded builds to an archive
    /// path; follow the move so an older catalog entry still installs.
    pub(super) async fn resolve_moved_url(
        &self,
        url: String,
        component: BinaryComponent,
    ) -> String {
        let Some(archive) = crate::runtime::php_catalog::windows_archive_url(&url)
            .or_else(|| crate::runtime::packages::mysql_archive_url(&url))
        else {
            return url;
        };
        let missing = self
            .build_head_request(&url, component)
            .send()
            .await
            .map(|response| response.status() == StatusCode::NOT_FOUND)
            .unwrap_or(false);
        if missing {
            eprintln!("{} moved to the archive, using {}", url, archive);
            archive
        } else {
            url
        }
    }

    pub(super) async fn probe_download_metadata(
        &self,
        url: &str,
        component: BinaryComponent,
    ) -> Option<DownloadMetadata> {
        if let Ok(response) = self.build_head_request(url, component).send().await {
            if response.status().is_success() {
                let total_bytes = response
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(0);
                let accepts_ranges = response
                    .headers()
                    .get(ACCEPT_RANGES)
                    .and_then(|value| value.to_str().ok())
                    .map(|value| value.eq_ignore_ascii_case("bytes"))
                    .unwrap_or(false);

                if total_bytes > 0 {
                    return Some(DownloadMetadata {
                        total_bytes,
                        accepts_ranges,
                    });
                }
            }
        }

        let response = self
            .build_download_request(url, component)
            .header(RANGE, "bytes=0-0")
            .send()
            .await
            .ok()?;

        if response.status() != StatusCode::PARTIAL_CONTENT {
            return None;
        }

        let total_bytes = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(Self::parse_content_range_total)
            .unwrap_or(0);

        if total_bytes == 0 {
            return None;
        }

        Some(DownloadMetadata {
            total_bytes,
            accepts_ranges: true,
        })
    }

    pub(super) fn parse_content_range_total(value: &str) -> Option<u64> {
        value.rsplit('/').next()?.parse::<u64>().ok()
    }

    pub(super) async fn should_use_segmented_download(
        &self,
        total_bytes: u64,
        accepts_ranges: bool,
        url: &str,
        component: BinaryComponent,
    ) -> bool {
        if total_bytes < SEGMENTED_DOWNLOAD_MIN_BYTES {
            return false;
        }

        if accepts_ranges {
            return true;
        }

        self.build_download_request(url, component)
            .header(RANGE, "bytes=0-0")
            .send()
            .await
            .map(|response| response.status() == StatusCode::PARTIAL_CONTENT)
            .unwrap_or(false)
    }

    pub(super) async fn download_component_segmented(
        &self,
        url: &str,
        component: BinaryComponent,
        partial_path: &Path,
        total_bytes: u64,
        progress_meta: &DownloadProgressMeta<'_>,
    ) -> Result<DownloadedFile, String> {
        let requested_channels = total_bytes
            .div_ceil(SEGMENTED_DOWNLOAD_MIN_PART_BYTES)
            .max(2) as usize;
        let channels = requested_channels.min(SEGMENTED_DOWNLOAD_MAX_CHANNELS);
        let part_size = total_bytes.div_ceil(channels as u64);
        let downloaded = Arc::new(AtomicU64::new(0));
        let mut join_set = JoinSet::new();
        let mut expected_segments = 0_usize;

        progress_meta.emit(0, 0, total_bytes);

        for index in 0..channels {
            let start = index as u64 * part_size;
            if start >= total_bytes {
                break;
            }
            let end = ((start + part_size).min(total_bytes)).saturating_sub(1);
            let part_path = partial_path.with_extension(format!("part.{}", index));
            let client = self.client.clone();
            let url = url.to_string();
            let user_agent = self.user_agent().to_string();
            let downloaded = Arc::clone(&downloaded);
            expected_segments += 1;

            join_set.spawn(async move {
                let _ = tokio::fs::remove_file(&part_path).await;
                let mut request = client
                    .get(&url)
                    .header("User-Agent", user_agent)
                    .header(RANGE, format!("bytes={}-{}", start, end));

                if component == BinaryComponent::MySQL && url.contains("dev.mysql.com") {
                    request = with_mysql_site_headers(request);
                }

                let mut response = request
                    .send()
                    .await
                    .map_err(|e| format!("Failed to fetch range {}-{}: {}", start, end, e))?;

                if response.status() != StatusCode::PARTIAL_CONTENT {
                    return Err(format!(
                        "Server did not honor range request {}-{}: HTTP {}",
                        start,
                        end,
                        response.status().as_u16()
                    ));
                }

                let file = tokio::fs::File::create(&part_path)
                    .await
                    .map_err(|e| format!("Failed to create part file: {}", e))?;
                let mut file = tokio::io::BufWriter::with_capacity(MERGE_BUFFER_BYTES, file);
                let mut part_bytes = 0_u64;

                while let Some(chunk) = response.chunk().await.map_err(|e| {
                    format!("Failed while downloading range {}-{}: {}", start, end, e)
                })? {
                    file.write_all(&chunk)
                        .await
                        .map_err(|e| format!("Failed to write part file: {}", e))?;
                    let chunk_len = chunk.len() as u64;
                    part_bytes = part_bytes.saturating_add(chunk_len);
                    downloaded.fetch_add(chunk_len, Ordering::Relaxed);
                }

                file.flush()
                    .await
                    .map_err(|e| format!("Failed to flush part file: {}", e))?;

                let expected_bytes = end - start + 1;
                if part_bytes != expected_bytes {
                    return Err(format!(
                        "Range {}-{} downloaded {} bytes, expected {} bytes",
                        start, end, part_bytes, expected_bytes
                    ));
                }

                Ok(SegmentDownloadResult {
                    index,
                    path: part_path,
                    bytes: part_bytes,
                })
            });
        }

        let mut results: Vec<Option<SegmentDownloadResult>> =
            (0..expected_segments).map(|_| None).collect();
        let mut completed_segments = 0_usize;
        let mut last_percent = u8::MAX;

        while completed_segments < expected_segments {
            tokio::select! {
                result = join_set.join_next() => {
                    let result = result
                        .ok_or_else(|| "Segmented downloader finished unexpectedly".to_string())?
                        .map_err(|e| format!("Segmented download task failed: {}", e))??;
                    let index = result.index;
                    if index >= results.len() {
                        return Err(format!("Segmented downloader returned invalid index {}", index));
                    }
                    results[index] = Some(result);
                    completed_segments += 1;
                }
                _ = tokio::time::sleep(PROGRESS_EMIT_INTERVAL) => {
                    let downloaded_bytes = downloaded.load(Ordering::Relaxed);
                    let percent = Self::download_percent(downloaded_bytes, total_bytes);
                    if percent != last_percent {
                        last_percent = percent;
                        progress_meta.emit(percent, downloaded_bytes, total_bytes);
                    }
                }
            }
        }

        let downloaded_bytes = downloaded.load(Ordering::Relaxed);
        progress_meta.emit(
            Self::download_percent(downloaded_bytes, total_bytes),
            downloaded_bytes,
            total_bytes,
        );

        self.merge_segmented_parts(partial_path, results, total_bytes)
    }

    pub(super) fn download_percent(downloaded_bytes: u64, total_bytes: u64) -> u8 {
        if total_bytes == 0 {
            return 0;
        }

        ((downloaded_bytes as f64 / total_bytes as f64) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    }

    pub(super) fn merge_segmented_parts(
        &self,
        partial_path: &Path,
        results: Vec<Option<SegmentDownloadResult>>,
        total_bytes: u64,
    ) -> Result<DownloadedFile, String> {
        let mut file = BufWriter::with_capacity(
            MERGE_BUFFER_BYTES,
            File::create(partial_path).map_err(|e| format!("Failed to create file: {}", e))?,
        );
        let mut hasher = Sha256::new();
        let mut magic_bytes = Vec::with_capacity(4);
        let mut downloaded_bytes = 0_u64;
        let mut buffer = vec![0_u8; MERGE_BUFFER_BYTES];

        for result in results {
            let result = result.ok_or_else(|| "Missing segmented download part".to_string())?;
            let mut part_file = BufReader::with_capacity(
                MERGE_BUFFER_BYTES,
                File::open(&result.path).map_err(|e| format!("Failed to open part file: {}", e))?,
            );
            let mut part_bytes = 0_u64;

            loop {
                let read = part_file
                    .read(&mut buffer)
                    .map_err(|e| format!("Failed to read part file: {}", e))?;
                if read == 0 {
                    break;
                }

                if magic_bytes.len() < 4 {
                    let remaining = 4 - magic_bytes.len();
                    magic_bytes.extend_from_slice(&buffer[..read.min(remaining)]);
                }

                hasher.update(&buffer[..read]);
                file.write_all(&buffer[..read])
                    .map_err(|e| format!("Failed to write merged file: {}", e))?;
                let read = read as u64;
                part_bytes = part_bytes.saturating_add(read);
                downloaded_bytes = downloaded_bytes.saturating_add(read);
            }

            if part_bytes != result.bytes {
                return Err(format!(
                    "Part {} changed while merging: read {} bytes, expected {} bytes",
                    result.index, part_bytes, result.bytes
                ));
            }

            let _ = fs::remove_file(&result.path);
        }

        file.flush()
            .map_err(|e| format!("Failed to flush downloaded file: {}", e))?;

        if downloaded_bytes != total_bytes {
            return Err(format!(
                "Merged download size mismatch: got {} bytes, expected {} bytes",
                downloaded_bytes, total_bytes
            ));
        }

        Ok(DownloadedFile {
            downloaded_bytes,
            total_bytes,
            magic_bytes,
            hasher,
        })
    }
}

pub(super) const SEGMENTED_DOWNLOAD_MIN_BYTES: u64 = 12 * 1024 * 1024;

pub(super) const SEGMENTED_DOWNLOAD_MIN_PART_BYTES: u64 = 8 * 1024 * 1024;

pub(super) const SEGMENTED_DOWNLOAD_MAX_CHANNELS: usize = 16;

pub(super) const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(250);

pub(super) fn with_mysql_site_headers(request: RequestBuilder) -> RequestBuilder {
    request
        .header("Referer", "https://dev.mysql.com/downloads/mysql/")
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
}
