//! Bound the logical result before the SDK creates its JSON text fallback.
use rmcp::model::CallToolResult;
use serde_json::{Value, json};
use std::io;

// The structured value and its escaped text fallback occupy up to roughly three
// copies of this encoded payload. Leave room below ordinary clients' 16 MiB line
// ceiling for protocol framing and ordinary request IDs.
const MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn encode(result: Result<Value, String>) -> CallToolResult {
    encode_with_limit(result, MAX_RESULT_BYTES)
}

fn encode_with_limit(result: Result<Value, String>, limit: usize) -> CallToolResult {
    let (value, failed) = match result {
        Ok(value) => (value, false),
        Err(error) => (json!({ "error": error }), true),
    };
    if serde_json::to_writer(Budget(limit), &value).is_err() {
        return CallToolResult::structured_error(json!({
            "error": "MCP result exceeds the output budget; request fewer rows or columns",
            "kind": "output_limit",
        }));
    }
    if failed {
        CallToolResult::structured_error(value)
    } else {
        CallToolResult::structured(value)
    }
}

struct Budget(usize);

impl io::Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.checked_sub(bytes.len()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "result byte budget exceeded")
        })?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
