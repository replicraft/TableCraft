//! Complete TSV batches share one cumulative transport allowance.
use crate::{commands::ExecutionError, output::TableOutput, presentation::Format};
use std::io::Write;
use tablecraft_core::values::Value;

pub(super) struct Publisher<'a> {
    writer: &'a mut dyn Write,
    pending: TableOutput,
    batch_rows: u32,
    pending_rows: u32,
    maximum_bytes: usize,
    published_bytes: usize,
    first: bool,
}

fn operation(error: impl std::fmt::Display) -> ExecutionError {
    ExecutionError::Operation(error.to_string())
}

impl<'a> Publisher<'a> {
    pub(super) fn new(
        names: &[String],
        batch_rows: u32,
        maximum_bytes: usize,
        writer: &'a mut dyn Write,
    ) -> Result<Self, ExecutionError> {
        let mut pending = TableOutput::with_limit(maximum_bytes);
        for (index, name) in names.iter().enumerate() {
            if index > 0 {
                pending.separator().map_err(operation)?;
            }
            pending.header(Some(name), index + 1).map_err(operation)?;
        }
        pending.newline().map_err(operation)?;

        Ok(Self {
            writer,
            pending,
            batch_rows,
            pending_rows: 0,
            maximum_bytes,
            published_bytes: 0,
            first: true,
        })
    }

    pub(super) fn append(&mut self, row: &[Value]) -> Result<(), ExecutionError> {
        for (index, value) in row.iter().enumerate() {
            if index > 0 {
                self.pending.separator().map_err(operation)?;
            }
            self.pending.value(value).map_err(operation)?;
        }
        self.pending.newline().map_err(operation)?;
        self.pending_rows += 1;
        if self.pending_rows == self.batch_rows {
            self.flush_batch()?;
        }
        Ok(())
    }

    /// Called only after checked source EOF; failed operations drop the pending batch.
    pub(super) fn finish(mut self) -> Result<(), ExecutionError> {
        if self.pending_rows > 0 || self.first {
            self.flush_batch()?;
        }
        Ok(())
    }

    fn flush_batch(&mut self) -> Result<(), ExecutionError> {
        let next = TableOutput::with_limit(self.maximum_bytes);
        let pending = std::mem::replace(&mut self.pending, next).finish();
        let remaining = self
            .maximum_bytes
            .checked_sub(self.published_bytes)
            .ok_or_else(|| operation("streaming output limit exceeded"))?;
        let encoded = Format::Tsv
            .render_with_limit(pending, remaining)
            .map_err(operation)?;
        self.published_bytes = self
            .published_bytes
            .checked_add(encoded.len())
            .filter(|bytes| *bytes <= self.maximum_bytes)
            .ok_or_else(|| operation("streaming output limit exceeded"))?;

        self.writer
            .write_all(encoded.as_bytes())
            .map_err(ExecutionError::Output)?;
        self.writer.flush().map_err(ExecutionError::Output)?;
        self.pending_rows = 0;
        self.first = false;
        Ok(())
    }
}

#[cfg(test)]
#[path = "publisher_tests.rs"]
mod tests;
