use crate::UpdateError;
use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::PublicKey;

include!(concat!(env!("OUT_DIR"), "/update_trust.rs"));

const MAX_KEY_BYTES: usize = 4096;

pub(crate) fn release_key() -> Result<PublicKey, UpdateError> {
    let text = RELEASE_PUBLIC_KEY.ok_or(UpdateError::NotConfigured)?;
    let label = text.replace(['-', '_'], " ").to_ascii_uppercase();
    if label.contains("TEST ONLY") {
        return Err(UpdateError::InvalidKey);
    }
    let key = parse_key(text)?;
    let raw = key_bytes(text)?;
    let test = key_bytes(include_str!("download/tests/TEST-ONLY.pub"))?;
    if raw[10..] == test[10..] {
        return Err(UpdateError::InvalidKey);
    }
    Ok(key)
}

pub(crate) fn parse_key(text: &str) -> Result<PublicKey, UpdateError> {
    key_bytes(text)?;
    let encoded = text.lines().nth(1).ok_or(UpdateError::InvalidKey)?;
    PublicKey::from_base64(encoded).map_err(|_| UpdateError::InvalidKey)
}

fn key_bytes(text: &str) -> Result<Vec<u8>, UpdateError> {
    if text.len() > MAX_KEY_BYTES || text.lines().count() != 2 {
        return Err(UpdateError::InvalidKey);
    }
    let mut lines = text.lines();
    if !lines
        .next()
        .is_some_and(|line| line.starts_with("untrusted comment:"))
    {
        return Err(UpdateError::InvalidKey);
    }
    let encoded = lines.next().ok_or(UpdateError::InvalidKey)?;
    let raw = STANDARD
        .decode(encoded)
        .map_err(|_| UpdateError::InvalidKey)?;
    if raw.len() != 42 || raw[..2] != *b"Ed" {
        return Err(UpdateError::InvalidKey);
    }
    Ok(raw)
}

#[cfg(test)]
mod tests;
