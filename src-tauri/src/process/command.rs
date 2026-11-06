//! Helpers for spawning background processes.

use std::process::Command;

/// Keep a console window from flashing up for a background process on Windows.
#[cfg(target_os = "windows")]
pub fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
pub fn hide_console(_command: &mut Command) {}
