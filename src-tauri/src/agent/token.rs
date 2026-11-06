//! The bearer token that protects the connector.

use crate::runtime::locator::get_app_data_paths;
use std::fs;
use std::path::PathBuf;

pub(super) fn token_path() -> Result<PathBuf, String> {
    Ok(get_app_data_paths()?.config_dir.join("agent-token"))
}

pub(super) fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("Failed to generate token: {}", e))?;
    Ok(hex::encode(bytes))
}

pub(super) fn write_token(token: &str) -> Result<(), String> {
    let path = token_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config folder: {}", e))?;
    }
    crate::fs_util::write_private(&path, token)
        .map_err(|e| format!("Failed to save agent token: {}", e))
}

pub(super) fn load_or_create_token() -> Result<String, String> {
    let path = token_path()?;
    if let Ok(token) = fs::read_to_string(&path) {
        let token = token.trim().to_string();
        if token.len() == 64 {
            return Ok(token);
        }
    }
    let token = generate_token()?;
    write_token(&token)?;
    Ok(token)
}

pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
