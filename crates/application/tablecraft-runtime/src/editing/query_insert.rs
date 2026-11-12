//! Lease-free SQL INSERT review owns the actual native plan, never UI authority.
use super::platform;
use crate::query::context::Environment;
use crate::{
    Error, InspectedQuery, ParameterInput, Response, display::Presentation, document::Document,
    query::parameters,
};
use std::path::Path;

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
    pub(super) state: Box<platform::query_insert::State>,
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
        environment: Environment<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(target, cancelled)?;
        let mut output = Presentation::new(cancelled);
        parameters::validate_input_limits(inputs, &mut output)?;
        platform::query_insert::review(self, target, inputs, environment, &mut output, cancelled)
    }
}
