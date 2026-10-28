//! Explicit progressive scalar TSV publication through one source-bound cursor.
use super::{Args, append_rows, display_rows};
use crate::{commands::ExecutionError, native, output::TableOutput, presentation::Format};
use std::io::Write;
use tablecraft_access::{BrowserCursor, NativeBrowser};
use tablecraft_core::limits::ResourceBudget;

const MAX_STREAM_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SELECTED_COLUMNS: usize = 256;

fn operation(error: impl std::fmt::Display) -> ExecutionError {
    ExecutionError::Operation(error.to_string())
}

impl Args {
    pub(in crate::commands) fn stream_rows(
        self,
        format: Format,
        password: Option<&str>,
        writer: &mut dyn Write,
    ) -> Result<(), ExecutionError> {
        let _span = tracing::debug_span!("cli.rows.stream").entered();
        if matches!(format, Format::Table) {
            return Err(operation("streaming rows require TSV; omit --format table"));
        }
        if self.columns.len() > MAX_SELECTED_COLUMNS {
            return Err(operation("too many selected streaming columns"));
        }
        let mut budget = ResourceBudget::new(
            self.native_bytes.unwrap_or(native::DEFAULT_NATIVE_BYTES),
            self.native_rows.unwrap_or(native::DEFAULT_NATIVE_ROWS),
            self.native_steps.unwrap_or(native::DEFAULT_NATIVE_STEPS),
        );
        let source = native::open(&self.path, password, &mut budget).map_err(operation)?;
        budget
            .charge_allocation(self.columns.len() as u64, size_of::<&str>() as u64)
            .map_err(operation)?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(self.columns.len())
            .map_err(|_| operation("streaming column allocation failed"))?;
        names.extend(self.columns.iter().map(String::as_str));
        let mut browser = NativeBrowser::new(source, native::CATALOG_SCOPE, budget, &|| false);
        let table =
            super::selected_table(&mut browser, self.table.as_deref()).map_err(operation)?;
        let mut cursor = if names.is_empty() {
            browser.cursor(&table)
        } else {
            browser.projected_cursor(&table, &names)
        }
        .map_err(operation)?;
        publish(&mut cursor, self.limit, MAX_STREAM_OUTPUT_BYTES, writer)
    }
}

fn publish<R: tablecraft_ace::NativeReadSource>(
    cursor: &mut BrowserCursor<'_, R>,
    batch_rows: u32,
    maximum_output: u64,
    writer: &mut dyn Write,
) -> Result<(), ExecutionError> {
    let mut first = true;
    let mut published = 0_u64;
    loop {
        let batch = cursor.next_page(batch_rows).map_err(operation)?;
        let text = if first {
            display_rows(
                cursor.columns().iter().map(|column| (column.name(), false)),
                batch.rows(),
            )
            .map_err(operation)?
        } else {
            let mut output = TableOutput::new();
            append_rows(&mut output, batch.rows()).map_err(operation)?;
            output.finish()
        };
        let encoded = Format::Tsv.render(text).map_err(operation)?;
        published = published
            .checked_add(encoded.len() as u64)
            .filter(|bytes| *bytes <= maximum_output)
            .ok_or_else(|| operation("streaming output limit exceeded"))?;
        writer
            .write_all(encoded.as_bytes())
            .map_err(ExecutionError::Output)?;
        writer.flush().map_err(ExecutionError::Output)?;
        first = false;
        if batch.reached_end() {
            tracing::debug!(
                rows = cursor.position(),
                bytes = published,
                "row stream completed"
            );
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
