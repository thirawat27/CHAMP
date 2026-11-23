//! Custom add-ons: services the user runs from their own binary (a newer
//! Redis, Valkey, an S3 server, a WebSocket hub, ...).
//!
//! CHAMP starts and stops them with the stack and waits for their port, but
//! never downloads, verifies, or updates the binary. They are always labeled
//! user-managed, and agents cannot create or edit them.

use super::AddonKind;
use crate::config::{AppSettings, CustomAddon};
use std::ffi::OsString;
use std::path::Path;

const MAX_ID_LEN: usize = 32;
const MAX_NAME_LEN: usize = 40;
const MAX_ARGS: usize = 32;
const MAX_ARG_LEN: usize = 512;

/// Lowercase letter first, then lowercase letters, digits, or inner hyphens.
fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_ID_LEN
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && bytes[bytes.len() - 1] != b'-'
}

/// Check a definition the user typed. The binary must exist now; it may
/// disappear later, which `check_runnable` reports at start.
pub(super) fn validate(addon: &CustomAddon) -> Result<(), String> {
    if !valid_id(&addon.id) {
        return Err(format!(
            "Invalid id `{}`. Use up to {MAX_ID_LEN} lowercase letters, digits, or hyphens, starting with a letter (for example `valkey`).",
            addon.id
        ));
    }
    if AddonKind::from_key(&addon.id).is_some() {
        return Err(format!(
            "`{}` is a built-in add-on. Choose another id.",
            addon.id
        ));
    }
    let name = addon.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN || name.contains(['\n', '\r']) {
        return Err(format!(
            "Give the add-on a name of 1-{MAX_NAME_LEN} characters."
        ));
    }
    if addon.port < 1024 {
        return Err(format!(
            "Port {} needs administrator rights. Use a port from 1024 to 65535.",
            addon.port
        ));
    }
    if addon.args.len() > MAX_ARGS || addon.args.iter().any(|arg| arg.len() > MAX_ARG_LEN) {
        return Err(format!(
            "Use at most {MAX_ARGS} arguments of up to {MAX_ARG_LEN} characters."
        ));
    }
    check_runnable(addon)
}

pub(super) fn check_runnable(addon: &CustomAddon) -> Result<(), String> {
    let command = Path::new(&addon.command);
    if !command.is_absolute() {
        return Err(format!(
            "{}: the program must be a full path (for example C:\\tools\\valkey\\valkey-server.exe).",
            addon.name
        ));
    }
    if !command.is_file() {
        return Err(format!(
            "{}: the program {} was not found.",
            addon.name,
            command.display()
        ));
    }
    Ok(())
}

/// Insert or replace `addon` (matched by id), validated with the whole
/// configuration so its port cannot clash with another service.
pub(super) fn upsert(settings: &mut AppSettings, mut addon: CustomAddon) -> Result<(), String> {
    addon.name = addon.name.trim().to_string();
    validate(&addon)?;
    let mut next = settings.clone();
    match next
        .addons
        .custom
        .iter_mut()
        .find(|saved| saved.id == addon.id)
    {
        Some(saved) => *saved = addon,
        None => next.addons.custom.push(addon),
    }
    next.validate().map_err(|errors| errors.join("; "))?;
    *settings = next;
    Ok(())
}

/// Arguments with `{port}` and `{data_dir}` filled in. Each argument goes to
/// the process as-is: no shell is involved.
pub(super) fn expand_args(addon: &CustomAddon, data_dir: &Path) -> Vec<OsString> {
    let data_dir = data_dir.to_string_lossy();
    addon
        .args
        .iter()
        .map(|arg| {
            OsString::from(
                arg.replace("{port}", &addon.port.to_string())
                    .replace("{data_dir}", &data_dir),
            )
        })
        .collect()
}

/// `CHAMP_<ID>` with hyphens as underscores: `my-s3` -> `CHAMP_MY_S3`.
pub(super) fn env_prefix(id: &str) -> String {
    format!("CHAMP_{}", id.to_ascii_uppercase().replace('-', "_"))
}
