use crate::runtime::locator::get_app_data_paths;
use install::{ensure_ngrok_installed, runtime_ngrok_paths};
use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

mod install;

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

/// One tunnel start at a time: the UI and agents can ask concurrently, and a
/// second agent would be untracked and rejected by ngrok (ERR_NGROK_108).
static TUNNEL_START: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Bumped by every stop, so a start still downloading ngrok knows it was
/// cancelled and does not open a public tunnel after the user pressed Stop.
static STOP_GENERATION: AtomicU64 = AtomicU64::new(0);

fn lock_runtime() -> Result<MutexGuard<'static, HttpsTunnelRuntime>, String> {
    tunnel_runtime()
        .lock()
        .map_err(|e| format!("Failed to acquire tunnel lock: {}", e))
}

fn tunnel_runtime() -> &'static Mutex<HttpsTunnelRuntime> {
    HTTPS_TUNNEL.get_or_init(|| Mutex::new(HttpsTunnelRuntime::default()))
}

pub async fn start_https_tunnel(web_port: u16) -> Result<HttpsTunnelStatus, String> {
    let local_url = local_url_for(web_port);
    let start_guard = TUNNEL_START.lock().await;

    if let Some(status) = running_status_for_local_url(&local_url)? {
        drop(start_guard);
        if status.ready {
            return Ok(status);
        }
        return wait_for_public_url().await;
    }

    stop_https_tunnel()?;
    let generation = STOP_GENERATION.load(Ordering::SeqCst);

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
    crate::process::command::hide_console(&mut command);

    let (stdout, stderr) = {
        // Holding the lock from here to storing the child means a stop either is
        // seen by this check or finds the new child and kills it.
        let mut runtime = lock_runtime()?;
        if STOP_GENERATION.load(Ordering::SeqCst) != generation {
            return Ok(runtime.status.clone());
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to start HTTPS tunnel: {}", e))?;
        crate::process::job::bind_child_to_app_lifetime(&child);
        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

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
        (stdout, stderr)
    };
    drop(start_guard);

    if let Some(stdout) = stdout {
        spawn_output_reader(stdout, log_path.clone());
    }
    if let Some(stderr) = stderr {
        spawn_output_reader(stderr, log_path.clone());
    }

    wait_for_public_url().await
}

pub fn stop_https_tunnel() -> Result<HttpsTunnelStatus, String> {
    STOP_GENERATION.fetch_add(1, Ordering::SeqCst);
    let mut runtime = lock_runtime()?;

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

fn local_url_for(web_port: u16) -> String {
    format!("http://127.0.0.1:{web_port}")
}

/// Caddy came up on `web_port`. A tunnel forwarding to any other port would keep
/// claiming to be ready while serving errors, so stop it.
pub fn stop_if_not_forwarding_to(web_port: u16) {
    let local_url = local_url_for(web_port);
    let stale = tunnel_runtime()
        .lock()
        .map(|runtime| runtime.child.is_some() && runtime.status.local_url != local_url)
        .unwrap_or(false);
    if stale {
        let _ = stop_https_tunnel();
    }
}

/// An agent orphaned by a crash would keep forwarding the public URL with the UI
/// showing no tunnel. Windows ends it with CHAMP through the job object; macOS and
/// Linux have no equivalent, so end it on the next launch.
pub fn spawn_stale_agent_cleanup() {
    if cfg!(target_os = "windows") {
        return;
    }
    let Ok(app_paths) = get_app_data_paths() else {
        return;
    };
    let executables: Vec<PathBuf> = runtime_ngrok_paths(&app_paths.runtime_dir)
        .into_iter()
        .filter(|path| path.exists())
        .collect();
    std::thread::spawn(move || {
        for executable in executables {
            crate::process::manager::stop_runtime_processes_by_executable(&executable, "ngrok");
        }
    });
}

pub fn get_https_tunnel_status() -> Result<HttpsTunnelStatus, String> {
    update_tunnel_health()?;
    let mut runtime = lock_runtime()?;
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
        crate::fs_util::write_private(&path, token)
            .map_err(|e| format!("Failed to save ngrok authtoken: {}", e))?;
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

fn running_status_for_local_url(local_url: &str) -> Result<Option<HttpsTunnelStatus>, String> {
    update_tunnel_health()?;
    let runtime = lock_runtime()?;
    if runtime.status.running && runtime.status.local_url == local_url {
        return Ok(Some(runtime.status.clone()));
    }
    Ok(None)
}

fn update_tunnel_health() -> Result<(), String> {
    let mut runtime = lock_runtime()?;

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
    let mut runtime = lock_runtime()?;
    runtime.status.ready = true;
    runtime.status.error = None;
    Ok(runtime.status.clone())
}

fn mark_public_url_timeout() -> Result<HttpsTunnelStatus, String> {
    let mut runtime = lock_runtime()?;
    runtime.status.ready = false;
    runtime.status.error = Some(
        "Timed out waiting for the public HTTPS domain to become reachable. Stop and start HTTPS to request a fresh tunnel."
            .to_string(),
    );
    Ok(runtime.status.clone())
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
            crate::logs::append_line(&log_path, &line);
            record_tunnel_output(&line);
        }
    });
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
///
/// Only that message counts: other lines can carry text from public requests,
/// and a crafted one must not replace the URL behind the Open button.
fn extract_ngrok_url(line: &str) -> Option<String> {
    if logfmt_value(line, "msg") != Some("started tunnel") {
        return None;
    }
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
mod tests;
