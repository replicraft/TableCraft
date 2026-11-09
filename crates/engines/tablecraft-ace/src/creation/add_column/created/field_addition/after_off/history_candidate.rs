//! Consuming append on an actual fully compared chronological source replay.
#[cfg(test)]
mod tests;

use super::{
    FieldAdditionTarget, check_request,
    history::{self, FieldAdditionAfterOffHistoryError as Error},
    kernel::{self, Replay},
    source::{ProvedImage, Source},
};
use crate::NativeImage;
use tablecraft_core::limits::ResourceBudget;

/// A repeated ADD candidate bound to a complete privately qualified replay image.
/// It has no constructor from a source-check result or arbitrary BEFORE/AFTER.
pub struct FieldAdditionAfterOffHistoryCandidate {
    proof: Replay,
    after: Vec<u8>,
    table: String,
    column: String,
    target: FieldAdditionTarget,
    columns: usize,
}

impl FieldAdditionAfterOffHistoryCandidate {
    /// Complete original replay bytes, fully compared to the caller's captured input.
    pub fn before_bytes(&self) -> &[u8] {
        self.proof.image().as_bytes()
    }

    /// Complete fitted native result, with no journal authority by itself.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Exact source-resolved table spelling.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Exact requested new field spelling.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Bound closed Number or compressed TEXT(255) declaration.
    pub const fn target(&self) -> FieldAdditionTarget {
        self.target
    }

    /// Resulting native field count, independent of the DB/property snapshot count.
    pub const fn column_count(&self) -> usize {
        self.columns
    }
}

impl NativeImage {
    /// Append a field after proving all current post-OFF field snapshots in order.
    /// Qualification, full original comparison and next append spend the same
    /// cumulative allowance. The source grammar and TCAC1–6 paths stay distinct.
    pub fn prepare_created_add_field_after_off_history_candidate(
        self,
        table: &str,
        column: &str,
        target: FieldAdditionTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FieldAdditionAfterOffHistoryCandidate, Error> {
        let _span =
            tracing::debug_span!("native.field_addition.after_off_history_candidate").entered();
        check_request(table, column, target, budget, cancelled)?;
        let proof = history::qualified_replay(&self, table, budget, cancelled)?;
        let source = Source::from_proof(proof, table, budget, cancelled)?;
        let built = kernel::append(source, column, target, budget, cancelled)?;
        Ok(FieldAdditionAfterOffHistoryCandidate {
            proof: built.proof,
            after: built.after,
            table: built.table,
            column: built.column,
            target,
            columns: built.columns,
        })
    }
}
