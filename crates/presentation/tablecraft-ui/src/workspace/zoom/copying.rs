//! Clipboard publication prepares one complete original value across UI frames.
use std::collections::TryReserveError;

const COPY_BYTES_PER_FRAME: usize = 64 * 1024;

#[derive(Default)]
pub(super) struct PendingCopy {
    text: String,
}

impl PendingCopy {
    /// The session revalidates its exact source before each call. UTF-8 chunk
    /// boundaries retain every source byte, including original line endings.
    pub fn advance(&mut self, value: &str) -> Result<bool, TryReserveError> {
        let start = self.text.len();
        let mut end = start.saturating_add(COPY_BYTES_PER_FRAME).min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        let chunk = &value[start..end];
        self.text.try_reserve(chunk.len())?;
        self.text.push_str(chunk);
        Ok(end == value.len())
    }

    pub fn finish(self) -> String {
        self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_work_keeps_utf8_boundaries_and_never_publishes_a_prefix() {
        let original = format!("{}\r\nEND", "🦀".repeat(40_000));
        let mut pending = PendingCopy::default();
        assert!(!pending.advance(&original).unwrap());
        assert!(pending.text.len() <= COPY_BYTES_PER_FRAME);
        assert!(original.starts_with(&pending.text));
        while !pending.advance(&original).unwrap() {}
        assert_eq!(pending.finish(), original);
    }
}
