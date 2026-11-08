//! Lease-free SQL INSERT review owns the actual native plan, never UI authority.
use crate::{
    Error, InspectedQuery, ParameterInput, Response, display::Presentation, document::Document,
    query::parameters,
};
use std::path::Path;
use tablecraft_expressions::functions::dates::clock::Clock;

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[path = "query_insert/native.rs"]
mod platform;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
#[path = "query_insert/unsupported.rs"]
mod platform;

/// Complete reviewed counts and display facts, not permission to write.
pub struct QueryInsertReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub inserted_rows: usize,
}

/// Runtime-issued source-bound INSERT, consumed by the nonreplaceable save worker.
/// Its native plan and invalidation token cannot be constructed by a host.
pub struct ReviewedQueryInsert {
    pub(super) state: Box<platform::State>,
}

impl ReviewedQueryInsert {
    pub fn source_path(&self) -> &Path {
        &self.state.target.path
    }

    pub fn table(&self) -> &str {
        &self.state.target.table
    }

    pub fn document_revision(&self) -> u64 {
        self.state.target.document_revision
    }
}

impl Document {
    pub(crate) fn review_query_insert(
        &mut self,
        target: &InspectedQuery,
        inputs: &[ParameterInput],
        clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(target, cancelled)?;
        let mut output = Presentation::new(cancelled);
        parameters::validate_input_limits(inputs, &mut output)?;
        platform::review(self, target, inputs, clock, &mut output, cancelled)
    }
}
