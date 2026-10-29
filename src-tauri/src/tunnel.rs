use crate::runtime::locator::get_app_data_paths;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const NGROK_COMPONENT: &str = "ngrok";
/// ngrok publishes only a rolling "v3-stable" download channel.
const NGROK_VERSION: &str = "v3-stable";
const NGROK_DOWNLOAD_BASE: &str = "https://bin.equinox.io/c/bNyj1mQVY4c";
/// TODO(retire S-02): the stable channel is unversioned, so no SHA-256 can be
/// pinned. Pin a versioned ngrok artifact with checksums, then set this to
/// `false`. A configured checksum that MISMATCHES must always hard-fail
/// regardless of this flag. Tracked in CODE_HEALTH_BACKLOG.md (S-02).
const ALLOW_UNVERIFIED_NGROK: bool = true;
const NGROK_AUTHTOKEN_FILE: &str = "ngrok-authtoken";
const NGROK_SIGNUP_URL: &str = "https://dashboard.ngrok.com/get-started/your-authtoken";
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
    /// An ngrok authtoken is saved in CHAMP, set in `NGROK_AUTHTOKEN`, or
    /// present in ngrok's own config file.
    pub authtoken_configured: bool,
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
                authtoken_configured: false,
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

    let saved_authtoken = read_saved_authtoken();
    if saved_authtoken.is_none() && !external_authtoken_configured() {
        return Err(format!(
            "ngrok needs a free authtoken. Sign in at {NGROK_SIGNUP_URL}, copy your authtoken, and save it in HTTPS Preview."
        ));
    }

    let app_paths = get_app_data_paths()?;
    fs::create_dir_all(&app_paths.runtime_dir)
        .map_err(|e| format!("Failed to create runtime directory: {}", e))?;
    fs::create_dir_all(&app_paths.logs_dir)
        .map_err(|e| format!("Failed to create logs directory: {}", e))?;

    let ngrok = ensure_ngrok_installed(&app_paths.runtime_dir).await?;
    let log_path = app_paths.logs_dir.join(TUNNEL_LOG_NAME);
    reset_tunnel_log(&log_path)?;

    let mut command = Command::new(&ngrok);
    command
        .args([
            "http",
            &local_url,
            "--log",
            "stdout",
            "--log-format",
            "logfmt",
        ])
        .current_dir(&app_paths.runtime_dir)
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Passed through the environment so the token never shows in process lists.
    if let Some(token) = saved_authtoken {
        command.env("NGROK_AUTHTOKEN", token);
    }
    configure_no_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start HTTPS tunnel: {}", e))?;
    crate::process::job::bind_child_to_app_lifetime(&child);
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
            authtoken_configured: true,
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
    runtime.status.authtoken_configured = authtoken_configured();
    Ok(runtime.status.clone())
}

pub fn get_https_tunnel_status() -> Result<HttpsTunnelStatus, String> {
    update_tunnel_health()?;
    let mut runtime = tunnel_runtime()
        .lock()
        .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))?;
    runtime.status.authtoken_configured = authtoken_configured();
    Ok(runtime.status.clone())
}

/// Save the ngrok authtoken CHAMP passes to the tunnel. An empty token clears it.
pub fn set_ngrok_authtoken(token: &str) -> Result<HttpsTunnelStatus, String> {
    let token = token.trim();
    let path = authtoken_path()?;
    if token.is_empty() {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove ngrok authtoken: {}", e))?;
        }
    } else {
        if !token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        {
            return Err("The ngrok authtoken contains unexpected characters.".to_string());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        fs::write(&path, token).map_err(|e| format!("Failed to save ngrok authtoken: {}", e))?;
        restrict_to_owner(&path)?;
    }
    get_https_tunnel_status()
}

fn authtoken_path() -> Result<PathBuf, String> {
    Ok(get_app_data_paths()?.config_dir.join(NGROK_AUTHTOKEN_FILE))
}

fn read_saved_authtoken() -> Option<String> {
    let token = fs::read_to_string(authtoken_path().ok()?).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

fn authtoken_configured() -> bool {
    read_saved_authtoken().is_some() || external_authtoken_configured()
}

/// A token from `NGROK_AUTHTOKEN` or from `ngrok config add-authtoken`.
fn external_authtoken_configured() -> bool {
    if std::env::var("NGROK_AUTHTOKEN").is_ok_and(|value| !value.trim().is_empty()) {
        return true;
    }
    ngrok_config_paths()
        .iter()
        .any(|path| fs::read_to_string(path).is_ok_and(|content| config_has_authtoken(&content)))
}

fn ngrok_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // ngrok v3: %LOCALAPPDATA%\ngrok, ~/Library/Application Support/ngrok, ~/.config/ngrok.
    if let Some(dir) = dirs::config_local_dir() {
        paths.push(dir.join("ngrok").join("ngrok.yml"));
    }
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config").join("ngrok").join("ngrok.yml"));
        paths.push(home.join(".ngrok2").join("ngrok.yml"));
    }
    paths
}

fn config_has_authtoken(content: &str) -> bool {
    content.lines().any(|line| {
        line.trim()
            .strip_prefix("authtoken:")
            .is_some_and(|value| !value.trim().trim_matches(['"', '\'']).is_empty())
    })
}

fn restrict_to_owner(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to protect ngrok authtoken: {}", e))?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
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

async fn ensure_ngrok_installed(runtime_dir: &Path) -> Result<PathBuf, String> {
    if let Some(path) = detect_ngrok(runtime_dir) {
        return Ok(path);
    }

    let install_dir = runtime_dir.join(NGROK_COMPONENT);
    fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create ngrok directory: {}", e))?;

    let target = install_dir.join(ngrok_executable_name());
    let download = ngrok_download()?;
    let temp = install_dir.join(download.temp_file_name);

    let bytes = reqwest::Client::builder()
        .user_agent("CHAMP HTTPS tunnel installer")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?
        .get(download.url.as_str())
        .send()
        .await
        .map_err(|e| format!("Failed to download ngrok: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Failed to download ngrok: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Failed to read ngrok download: {}", e))?;

    // Checksum verification is fail-closed (S-02): a configured SHA-256 that
    // mismatches always hard-fails. When none is configured, the install is
    // refused unless ALLOW_UNVERIFIED_NGROK is explicitly set.
    match download.sha256 {
        Some(expected) => {
            let actual = hex::encode(Sha256::digest(&bytes));
            if actual.to_lowercase() != expected.to_lowercase() {
                return Err(format!(
                    "Checksum verification failed for ngrok {}.\nExpected: {}\nActual: {}\n\nThe downloaded file may be corrupted or tampered with.",
                    NGROK_VERSION, expected, actual
                ));
            }
            eprintln!("Checksum verified for ngrok {NGROK_VERSION}: {actual}");
        }
        None if ALLOW_UNVERIFIED_NGROK => {
            eprintln!(
                "WARNING: no checksum configured for ngrok {NGROK_VERSION} — installing UNVERIFIED."
            );
        }
        None => {
            return Err(format!(
                "No SHA-256 checksum is configured for ngrok {NGROK_VERSION}. \
                 Installation was refused because the downloaded binary cannot be verified.",
            ));
        }
    }

    fs::write(&temp, bytes).map_err(|e| format!("Failed to save ngrok: {}", e))?;
    let extracted = match download.archive {
        NgrokArchive::Zip => extract_ngrok_zip(&temp, &target),
        NgrokArchive::Tgz => extract_ngrok_tgz(&temp, &target),
    };
    let _ = fs::remove_file(&temp);
    extracted?;

    make_executable(&target)?;
    fs::write(
        runtime_dir.join("ngrok_installed.txt"),
        format!(
            "version={NGROK_VERSION}\ninstalled_at={:?}\n",
            SystemTime::now()
        ),
    )
    .map_err(|e| format!("Failed to write ngrok marker: {}", e))?;

    Ok(target)
}

fn detect_ngrok(runtime_dir: &Path) -> Option<PathBuf> {
    let executable = ngrok_executable_name();
    [
        runtime_dir.join(NGROK_COMPONENT).join(executable),
        runtime_dir.join(executable),
        runtime_dir.join("bin").join(executable),
    ]
    .into_iter()
    .find(|path| path.exists())
    .or_else(system_ngrok)
}

fn system_ngrok() -> Option<PathBuf> {
    let mut command = Command::new("ngrok");
    command
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_no_window(&mut command);
    let output = command.output().ok()?;
    output.status.success().then(|| PathBuf::from("ngrok"))
}

enum NgrokArchive {
    Zip,
    Tgz,
}

struct NgrokDownload {
    url: String,
    temp_file_name: &'static str,
    archive: NgrokArchive,
    /// Expected SHA-256 of the downloaded artifact for this platform.
    /// `None` while CHAMP installs the unversioned stable channel (S-02).
    sha256: Option<&'static str>,
}

fn ngrok_download() -> Result<NgrokDownload, String> {
    let (platform, archive) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => ("windows-amd64", NgrokArchive::Zip),
        ("windows", "aarch64") => ("windows-arm64", NgrokArchive::Zip),
        ("linux", "x86_64") => ("linux-amd64", NgrokArchive::Tgz),
        ("linux", "aarch64") => ("linux-arm64", NgrokArchive::Tgz),
        ("macos", "x86_64") => ("darwin-amd64", NgrokArchive::Zip),
        ("macos", "aarch64") => ("darwin-arm64", NgrokArchive::Zip),
        (os, arch) => {
            return Err(format!(
                "Automatic HTTPS tunnel runtime is not available for {os}/{arch}"
            ))
        }
    };
    let (extension, temp_file_name) = match archive {
        NgrokArchive::Zip => ("zip", "ngrok-download.zip"),
        NgrokArchive::Tgz => ("tgz", "ngrok-download.tgz"),
    };
    Ok(NgrokDownload {
        url: format!("{NGROK_DOWNLOAD_BASE}/ngrok-{NGROK_VERSION}-{platform}.{extension}"),
        temp_file_name,
        archive,
        sha256: None,
    })
}

fn ngrok_executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ngrok.exe"
    } else {
        "ngrok"
    }
}

fn extract_ngrok_zip(archive_path: &Path, target: &Path) -> Result<(), String> {
    let file =
        File::open(archive_path).map_err(|e| format!("Failed to open ngrok archive: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ngrok archive: {}", e))?;
    let mut entry = archive
        .by_name(ngrok_executable_name())
        .map_err(|_| "ngrok binary was not found in the downloaded archive".to_string())?;
    let mut output =
        File::create(target).map_err(|e| format!("Failed to create ngrok binary: {}", e))?;
    std::io::copy(&mut entry, &mut output)
        .map_err(|e| format!("Failed to extract ngrok: {}", e))?;
    Ok(())
}

fn extract_ngrok_tgz(archive_path: &Path, target: &Path) -> Result<(), String> {
    let file =
        File::open(archive_path).map_err(|e| format!("Failed to open ngrok archive: {}", e))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to inspect ngrok archive: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read ngrok archive: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to read ngrok archive path: {}", e))?;
        if path.file_name().and_then(|name| name.to_str()) == Some("ngrok") {
            let mut output = File::create(target)
                .map_err(|e| format!("Failed to create ngrok binary: {}", e))?;
            std::io::copy(&mut entry, &mut output)
                .map_err(|e| format!("Failed to extract ngrok: {}", e))?;
            return Ok(());
        }
    }

    Err("ngrok binary was not found in the downloaded archive".to_string())
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|e| format!("Failed to read ngrok permissions: {}", e))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .map_err(|e| format!("Failed to mark ngrok executable: {}", e))?;
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
    let public_url = extract_ngrok_url(line);
    let error = extract_ngrok_error(line);

    if public_url.is_none() && error.is_none() {
        return;
    }

    if let Ok(mut runtime) = tunnel_runtime().lock() {
        if let Some(url) = public_url {
            runtime.status.url = Some(url);
            runtime.status.ready = false;
            runtime.status.error = None;
        } else if runtime.status.url.is_none() {
            runtime.status.error = error;
        }
    }
}

/// Value of a `key=value` / `key="quoted value"` field in an ngrok logfmt line.
fn logfmt_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}=");
    let start = line
        .match_indices(&needle)
        .find(|(index, _)| *index == 0 || line.as_bytes()[index - 1] == b' ')?
        .0
        + needle.len();
    let rest = &line[start..];
    if let Some(quoted) = rest.strip_prefix('"') {
        Some(&quoted[..quoted.find('"').unwrap_or(quoted.len())])
    } else {
        Some(&rest[..rest.find(char::is_whitespace).unwrap_or(rest.len())])
    }
}

/// `msg="started tunnel" ... url=https://<name>.ngrok-free.app`
fn extract_ngrok_url(line: &str) -> Option<String> {
    let url = logfmt_value(line, "url")?;
    url.starts_with("https://").then(|| url.to_string())
}

fn extract_ngrok_error(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let is_error = lower.contains("lvl=eror")
        || lower.contains("lvl=crit")
        || lower.trim_start().starts_with("error:")
        || lower.contains("err_ngrok_");
    if !is_error {
        return None;
    }

    let message = logfmt_value(line, "err")
        .filter(|value| !value.is_empty() && *value != "nil")
        .unwrap_or(line.trim())
        .trim_start_matches("ERROR:")
        .trim()
        .to_string();

    // ERR_NGROK_4018: missing token, ERR_NGROK_105/107: invalid or revoked token.
    if ["err_ngrok_4018", "err_ngrok_105", "err_ngrok_107"]
        .iter()
        .any(|code| lower.contains(code))
    {
        return Some(format!(
            "ngrok rejected the authtoken. Copy a valid token from {NGROK_SIGNUP_URL} and save it in HTTPS Preview. ({message})"
        ));
    }
    // ERR_NGROK_108: the free plan allows one agent session at a time.
    if lower.contains("err_ngrok_108") {
        return Some(format!(
            "Another ngrok agent is already online with this account. Stop it and start HTTPS again. ({message})"
        ));
    }
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_ngrok_url_from_started_tunnel_line() {
        let line = r#"t=2026-10-28T21:00:00+0700 lvl=info msg="started tunnel" obj=tunnels name=command_line addr=http://127.0.0.1:8080 url=https://demo-random.ngrok-free.app"#;
        assert_eq!(
            extract_ngrok_url(line).as_deref(),
            Some("https://demo-random.ngrok-free.app")
        );
    }

    #[test]
    fn ignores_lines_without_public_url() {
        assert!(extract_ngrok_url("lvl=info msg=\"client session established\"").is_none());
        assert!(extract_ngrok_url("lvl=info addr=http://127.0.0.1:8080").is_none());
    }

    #[test]
    fn explains_missing_authtoken() {
        let line = r#"t=2026-10-28T21:00:00+0700 lvl=eror msg="failed to reconnect session" obj=tunnels.session err="authentication failed: Usage of ngrok requires a verified account and authtoken.\n\nERR_NGROK_4018\n""#;
        let error = extract_ngrok_error(line).unwrap();
        assert!(error.contains("authtoken"));
        assert!(error.contains(NGROK_SIGNUP_URL));
    }

    #[test]
    fn ignores_info_lines_as_errors() {
        assert!(extract_ngrok_error("lvl=info msg=\"no configuration paths supplied\"").is_none());
    }

    #[test]
    fn reads_authtoken_from_ngrok_config() {
        assert!(config_has_authtoken(
            "version: \"3\"\nagent:\n    authtoken: abc123\n"
        ));
        assert!(!config_has_authtoken(
            "version: \"3\"\nagent:\n    authtoken: \"\"\n"
        ));
    }
}
