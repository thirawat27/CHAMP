use crate::runtime::locator::get_app_data_paths;
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const CLOUDFLARED_COMPONENT: &str = "cloudflared";
const TUNNEL_LOG_NAME: &str = "https-tunnel.log";
const PUBLIC_URL_WAIT_ATTEMPTS: usize = 120;
const PUBLIC_URL_WAIT_DELAY: Duration = Duration::from_millis(250);
const PUBLIC_URL_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
pub struct HttpsTunnelStatus {
    pub running: bool,
    pub url: Option<String>,
    pub ready: bool,
    pub local_url: String,
    pub error: Option<String>,
    pub log_path: Option<String>,
    pub pid: Option<u32>,
}

struct HttpsTunnelRuntime {
    child: Option<Child>,
    status: HttpsTunnelStatus,
}

impl Default for HttpsTunnelRuntime {
    fn default() -> Self {
        Self {
            child: None,
            status: HttpsTunnelStatus {
                running: false,
                url: None,
                ready: false,
                local_url: String::new(),
                error: None,
                log_path: None,
                pid: None,
            },
        }
    }
}

static HTTPS_TUNNEL: OnceLock<Mutex<HttpsTunnelRuntime>> = OnceLock::new();

fn tunnel_runtime() -> &'static Mutex<HttpsTunnelRuntime> {
    HTTPS_TUNNEL.get_or_init(|| Mutex::new(HttpsTunnelRuntime::default()))
}

#[cfg(target_os = "windows")]
fn configure_no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn configure_no_window(_command: &mut Command) {}

pub async fn start_https_tunnel(web_port: u16) -> Result<HttpsTunnelStatus, String> {
    let local_url = format!("http://127.0.0.1:{web_port}");

    if let Some(status) = running_status_for_local_url(&local_url)? {
        if status.ready {
            return Ok(status);
        }
        return wait_for_public_url().await;
    }

    stop_https_tunnel()?;

    let app_paths = get_app_data_paths()?;
    fs::create_dir_all(&app_paths.runtime_dir)
        .map_err(|e| format!("Failed to create runtime directory: {}", e))?;
    fs::create_dir_all(&app_paths.logs_dir)
        .map_err(|e| format!("Failed to create logs directory: {}", e))?;

    let cloudflared = ensure_cloudflared_installed(&app_paths.runtime_dir).await?;
    let log_path = app_paths.logs_dir.join(TUNNEL_LOG_NAME);
    reset_tunnel_log(&log_path)?;

    let mut command = Command::new(&cloudflared);
    command
        .args(["tunnel", "--no-autoupdate", "--url", &local_url])
        .current_dir(&app_paths.runtime_dir)
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_no_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start HTTPS tunnel: {}", e))?;
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut runtime = tunnel_runtime()
            .lock()
            .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))?;
        runtime.status = HttpsTunnelStatus {
            running: true,
            url: None,
            ready: false,
            local_url: local_url.clone(),
            error: None,
            log_path: Some(log_path.to_string_lossy().to_string()),
            pid: Some(pid),
        };
        runtime.child = Some(child);
    }

    if let Some(stdout) = stdout {
        spawn_output_reader(stdout, log_path.clone());
    }
    if let Some(stderr) = stderr {
        spawn_output_reader(stderr, log_path.clone());
    }

    wait_for_public_url().await
}

pub fn stop_https_tunnel() -> Result<HttpsTunnelStatus, String> {
    let mut runtime = tunnel_runtime()
        .lock()
        .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))?;

    if let Some(mut child) = runtime.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    runtime.status.running = false;
    runtime.status.pid = None;
    runtime.status.url = None;
    runtime.status.ready = false;
    runtime.status.error = None;
    Ok(runtime.status.clone())
}

pub fn get_https_tunnel_status() -> Result<HttpsTunnelStatus, String> {
    update_tunnel_health()?;
    let runtime = tunnel_runtime()
        .lock()
        .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))?;
    Ok(runtime.status.clone())
}

fn running_status_for_local_url(local_url: &str) -> Result<Option<HttpsTunnelStatus>, String> {
    update_tunnel_health()?;
    let runtime = tunnel_runtime()
        .lock()
        .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))?;
    if runtime.status.running && runtime.status.local_url == local_url {
        return Ok(Some(runtime.status.clone()));
    }
    Ok(None)
}

fn update_tunnel_health() -> Result<(), String> {
    let mut runtime = tunnel_runtime()
        .lock()
        .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))?;

    if let Some(child) = runtime.child.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                runtime.child = None;
                runtime.status.running = false;
                runtime.status.pid = None;
                runtime.status.ready = false;
                if runtime.status.error.is_none() {
                    runtime.status.error = Some(format!("HTTPS tunnel exited with {}", status));
                }
            }
            Ok(None) => {
                runtime.status.running = true;
            }
            Err(e) => {
                runtime.status.running = false;
                runtime.status.ready = false;
                runtime.status.error = Some(format!("Failed to check HTTPS tunnel: {}", e));
            }
        }
    } else {
        runtime.status.running = false;
        runtime.status.pid = None;
        runtime.status.ready = false;
    }

    Ok(())
}

async fn wait_for_public_url() -> Result<HttpsTunnelStatus, String> {
    let client = reqwest::Client::builder()
        .user_agent("CHAMP HTTPS tunnel readiness probe")
        .timeout(PUBLIC_URL_PROBE_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to create HTTPS probe client: {}", e))?;

    for _ in 0..PUBLIC_URL_WAIT_ATTEMPTS {
        let status = get_https_tunnel_status()?;
        if !status.running {
            return Ok(status);
        }

        if let Some(url) = status.url.as_deref() {
            if is_public_url_ready(&client, url).await {
                return mark_public_url_ready();
            }
        }

        tokio::time::sleep(PUBLIC_URL_WAIT_DELAY).await;
    }

    mark_public_url_timeout()
}

async fn is_public_url_ready(client: &reqwest::Client, url: &str) -> bool {
    client
        .get(url)
        .send()
        .await
        .map(|response| response.status().as_u16() < 500)
        .unwrap_or(false)
}

fn mark_public_url_ready() -> Result<HttpsTunnelStatus, String> {
    let mut runtime = tunnel_runtime()
        .lock()
        .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))?;
    runtime.status.ready = true;
    runtime.status.error = None;
    Ok(runtime.status.clone())
}

fn mark_public_url_timeout() -> Result<HttpsTunnelStatus, String> {
    let mut runtime = tunnel_runtime()
        .lock()
        .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))?;
    runtime.status.ready = false;
    runtime.status.error = Some(
        "Timed out waiting for the public HTTPS domain to become reachable. Stop and start HTTPS to request a fresh tunnel."
            .to_string(),
    );
    Ok(runtime.status.clone())
}

async fn ensure_cloudflared_installed(runtime_dir: &Path) -> Result<PathBuf, String> {
    if let Some(path) = detect_cloudflared(runtime_dir) {
        return Ok(path);
    }

    let install_dir = runtime_dir.join(CLOUDFLARED_COMPONENT);
    fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create cloudflared directory: {}", e))?;

    let target = install_dir.join(cloudflared_executable_name());
    let download = cloudflared_download()?;
    let temp = install_dir.join(download.temp_file_name);

    let bytes = reqwest::Client::builder()
        .user_agent("CHAMP HTTPS tunnel installer")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?
        .get(download.url)
        .send()
        .await
        .map_err(|e| format!("Failed to download cloudflared: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Failed to download cloudflared: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Failed to read cloudflared download: {}", e))?;

    fs::write(&temp, bytes).map_err(|e| format!("Failed to save cloudflared: {}", e))?;
    if download.archive_tgz {
        extract_cloudflared_tgz(&temp, &target)?;
        let _ = fs::remove_file(&temp);
    } else {
        if target.exists() {
            fs::remove_file(&target)
                .map_err(|e| format!("Failed to replace cloudflared: {}", e))?;
        }
        fs::rename(&temp, &target).map_err(|e| format!("Failed to install cloudflared: {}", e))?;
    }

    make_executable(&target)?;
    fs::write(
        runtime_dir.join("cloudflared_installed.txt"),
        format!("version=latest\ninstalled_at={:?}\n", SystemTime::now()),
    )
    .map_err(|e| format!("Failed to write cloudflared marker: {}", e))?;

    Ok(target)
}

fn detect_cloudflared(runtime_dir: &Path) -> Option<PathBuf> {
    let executable = cloudflared_executable_name();
    [
        runtime_dir.join(CLOUDFLARED_COMPONENT).join(executable),
        runtime_dir.join(executable),
        runtime_dir.join("bin").join(executable),
    ]
    .into_iter()
    .find(|path| path.exists())
    .or_else(system_cloudflared)
}

fn system_cloudflared() -> Option<PathBuf> {
    let output = Command::new("cloudflared")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from("cloudflared"))
}

struct CloudflaredDownload {
    url: &'static str,
    temp_file_name: &'static str,
    archive_tgz: bool,
}

fn cloudflared_download() -> Result<CloudflaredDownload, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok(CloudflaredDownload {
            url: "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe",
            temp_file_name: "cloudflared-download.exe",
            archive_tgz: false,
        }),
        ("windows", "aarch64") => Ok(CloudflaredDownload {
            url: "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-arm64.exe",
            temp_file_name: "cloudflared-download.exe",
            archive_tgz: false,
        }),
        ("linux", "x86_64") => Ok(CloudflaredDownload {
            url: "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64",
            temp_file_name: "cloudflared-download",
            archive_tgz: false,
        }),
        ("linux", "aarch64") => Ok(CloudflaredDownload {
            url: "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-arm64",
            temp_file_name: "cloudflared-download",
            archive_tgz: false,
        }),
        ("macos", "x86_64") => Ok(CloudflaredDownload {
            url: "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-amd64.tgz",
            temp_file_name: "cloudflared-download.tgz",
            archive_tgz: true,
        }),
        ("macos", "aarch64") => Ok(CloudflaredDownload {
            url: "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-arm64.tgz",
            temp_file_name: "cloudflared-download.tgz",
            archive_tgz: true,
        }),
        (os, arch) => Err(format!(
            "Automatic HTTPS tunnel runtime is not available for {os}/{arch}"
        )),
    }
}

fn cloudflared_executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "cloudflared.exe"
    } else {
        "cloudflared"
    }
}

fn extract_cloudflared_tgz(archive_path: &Path, target: &Path) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|e| format!("Failed to open cloudflared archive: {}", e))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to inspect cloudflared archive: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read cloudflared archive: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to read cloudflared archive path: {}", e))?;
        if path.file_name().and_then(|name| name.to_str()) == Some("cloudflared") {
            let mut output = File::create(target)
                .map_err(|e| format!("Failed to create cloudflared binary: {}", e))?;
            std::io::copy(&mut entry, &mut output)
                .map_err(|e| format!("Failed to extract cloudflared: {}", e))?;
            return Ok(());
        }
    }

    Err("cloudflared binary was not found in the downloaded archive".to_string())
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|e| format!("Failed to read cloudflared permissions: {}", e))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .map_err(|e| format!("Failed to mark cloudflared executable: {}", e))?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

fn reset_tunnel_log(log_path: &Path) -> Result<(), String> {
    let mut file =
        File::create(log_path).map_err(|e| format!("Failed to create HTTPS tunnel log: {}", e))?;
    writeln!(file, "CHAMP HTTPS tunnel log")
        .map_err(|e| format!("Failed to write HTTPS tunnel log: {}", e))
}

fn spawn_output_reader<R>(reader: R, log_path: PathBuf)
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            append_log_line(&log_path, &line);
            record_tunnel_output(&line);
        }
    });
}

fn append_log_line(log_path: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{line}");
    }
}

fn record_tunnel_output(line: &str) {
    let public_url = extract_trycloudflare_url(line);
    let lower = line.to_ascii_lowercase();
    let is_error = lower.contains("error") || lower.contains("failed") || lower.contains("panic");

    if public_url.is_none() && !is_error {
        return;
    }

    if let Ok(mut runtime) = tunnel_runtime().lock() {
        if let Some(url) = public_url {
            runtime.status.url = Some(url);
            runtime.status.ready = false;
            runtime.status.error = None;
        } else if runtime.status.url.is_none() {
            runtime.status.error = Some(line.trim().to_string());
        }
    }
}

fn extract_trycloudflare_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let tail = &line[start..];
    let end = tail
        .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | '<' | '>' | ')' | ']'))
        .unwrap_or(tail.len());
    let url = tail[..end]
        .trim_end_matches(['.', ',', ';', ':'])
        .to_string();

    (url.contains(".trycloudflare.com") || url.ends_with("trycloudflare.com")).then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_trycloudflare_url_from_log_line() {
        let line = "Visit it at: https://demo-random.trycloudflare.com";
        assert_eq!(
            extract_trycloudflare_url(line).as_deref(),
            Some("https://demo-random.trycloudflare.com")
        );
    }

    #[test]
    fn ignores_non_trycloudflare_https_urls() {
        assert!(extract_trycloudflare_url("docs: https://example.com").is_none());
    }
}
