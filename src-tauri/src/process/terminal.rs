//! Opening a terminal window with the CHAMP runtimes on `PATH`.

use std::path::Path;

#[cfg(target_os = "windows")]
pub fn spawn_terminal(target_dir: &Path, path_env: &std::ffi::OsStr) -> Result<(), String> {
    // Absolute: `start` searches the working directory first, and that is the
    // projects folder here, so a file named cmd.exe in it would run instead.
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let cmd = std::path::PathBuf::from(system_root)
        .join("System32")
        .join("cmd.exe");

    std::process::Command::new(&cmd)
        // The empty string is `start`'s window title; a quoted path would be taken
        // for the title without it.
        .args(["/c", "start", ""])
        .arg(&cmd)
        .env("PATH", path_env)
        .current_dir(target_dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open terminal: {}", e))
}

#[cfg(target_os = "macos")]
pub fn spawn_terminal(target_dir: &Path, path_env: &std::ffi::OsStr) -> Result<(), String> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    // Terminal.app does not inherit our environment, so hand it a tiny script
    // that sets PATH and the working directory before starting the user's shell.
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    let script = format!(
        "#!/bin/sh\ncd {}\nexport PATH={}\nexec \"${{SHELL:-/bin/zsh}}\" -i\n",
        quote(&target_dir.to_string_lossy()),
        quote(&path_env.to_string_lossy()),
    );
    let script_path = std::env::temp_dir().join("champ-terminal.command");
    fs::write(&script_path, script)
        .map_err(|e| format!("Failed to prepare terminal script: {}", e))?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("Failed to prepare terminal script: {}", e))?;

    std::process::Command::new("open")
        .args(["-a", "Terminal"])
        .arg(&script_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open terminal: {}", e))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn spawn_terminal(target_dir: &Path, path_env: &std::ffi::OsStr) -> Result<(), String> {
    // Most emulators start in the spawning process' working directory, which
    // avoids relying on emulator-specific flags such as --working-directory.
    const TERMINALS: [&str; 6] = [
        "x-terminal-emulator",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "kitty",
        "xterm",
    ];

    let mut last_error = String::from("no terminal emulator found");
    for terminal in TERMINALS {
        match std::process::Command::new(terminal)
            .env("PATH", path_env)
            .current_dir(target_dir)
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(e) => last_error = format!("{}: {}", terminal, e),
        }
    }
    Err(format!("Failed to open terminal ({})", last_error))
}
