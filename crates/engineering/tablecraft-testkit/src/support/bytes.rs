//! Renderer-independent text for exact fixture and artifact byte observations.

/// Render complete bytes as lowercase hexadecimal for literal fixture pins.
/// This helper does not hash, decode or construct the observed database bytes.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
