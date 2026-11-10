use super::{FirstTableDefinition, source::require_canonical_blank};
use crate::{
    CreateDatabaseError, CreatedDatabase, InspectError, NativeImage, create_ace16_database,
    header::check,
};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

/// Failure before a canonical blank receives its first explicit table schema.
#[derive(Debug, Error)]
pub enum FirstTableError {
    /// Bounded source, allocation or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The original declaration is outside the constructive schema profile.
    #[error(transparent)]
    Create(#[from] CreateDatabaseError),
    /// The complete original bytes differ from the canonical blank profile.
    #[error("first-table creation requires the exact canonical blank database")]
    NotCanonicalBlank,
}

/// A complete canonical-blank-to-first-table schema plan, distinct from row edits.
/// Its original bytes must still match the retained target at journal preparation.
/// This type never authorizes replacement of an arbitrary imported database.
pub struct FirstTablePlan {
    before: NativeImage,
    after: CreatedDatabase,
    definition: FirstTableDefinition,
}

impl NativeImage {
    /// Prove the exact canonical blank source and serialize its first empty table.
    /// This full-image schema operation grants no general CREATE TABLE, field
    /// alteration, imported-object preservation or native Access save authority.
    pub fn plan_first_table(
        self,
        definition: FirstTableDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FirstTablePlan, FirstTableError> {
        let _span = tracing::debug_span!("native.first_table.plan").entered();
        check(budget, cancelled)?;
        if definition.is_native() {
            return Err(CreateDatabaseError::Declaration.into());
        }
        self.require_standard_capture()?;
        require_canonical_blank(self.as_bytes(), budget, cancelled)?;
        let columns = definition.borrowed_columns(budget, cancelled)?;
        let after = create_ace16_database(definition.table(), &columns, &[], budget, cancelled)?;
        drop(columns);
        check(budget, cancelled)?;
        Ok(FirstTablePlan {
            before: self,
            after,
            definition,
        })
    }
}

impl FirstTablePlan {
    /// Complete unchanged original canonical blank bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete directly serialized replacement with one empty user table.
    pub fn after_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }

    /// Exact proposed native table name.
    pub fn table(&self) -> &str {
        self.definition.table()
    }

    /// Number of declarations in the new empty table, not a changed-row count.
    pub fn column_count(&self) -> usize {
        self.definition.column_count()
    }

    /// Discard the staged images while retaining the original owned declaration.
    /// A later apply must capture and qualify its actual target again.
    pub fn into_definition(self) -> FirstTableDefinition {
        self.definition
    }

    #[cfg(feature = "journal")]
    pub(crate) fn definition(&self) -> &FirstTableDefinition {
        &self.definition
    }
}
