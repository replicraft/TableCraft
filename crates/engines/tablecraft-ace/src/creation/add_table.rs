//! Append one empty Increment table while retaining existing native identities.
mod audit;
mod availability;
mod build;
mod geometry;
mod retained;
pub(in crate::creation) mod source;

use super::{CreateDatabaseError, FirstTableDefinition};
use crate::{InspectError, NativeImage, NativeImageSource, header::check};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

impl From<super::catalog_page::PageError> for AddTableError {
    fn from(error: super::catalog_page::PageError) -> Self {
        match error {
            super::catalog_page::PageError::Inspect(error) => Self::Inspect(error),
            super::catalog_page::PageError::Source => Self::Source,
            super::catalog_page::PageError::Capacity => Self::Capacity,
        }
    }
}

pub use availability::{
    AddTableSourceCapabilities, add_table_source_available, add_table_source_capabilities,
};

/// Refusal before an existing source receives a new table.
#[derive(Debug, Error)]
pub enum AddTableError {
    /// Inspection, allocation, resource exhaustion or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The proposed empty schema fails its existing constructive declaration rules.
    #[error(transparent)]
    Create(#[from] CreateDatabaseError),
    /// Original catalog, ownership or file geometry is outside the admitted profile.
    #[error("database is outside the append-table source profile")]
    Source,
    /// A catalog name conflicts under its owning ASCII-insensitive name policy.
    #[error("table name already exists")]
    DuplicateName,
    /// The new records cannot fit without moving existing entries or growing maps.
    #[error("catalog or usage-map append capacity is exhausted")]
    Capacity,
    /// Independent original-byte or decoded new-role verification refused the candidate.
    #[error("append-table preservation check failed")]
    Preservation,
}

/// A checked append plan, separate from filesystem ownership and recovery authority.
/// Existing records, primary roots and physical references remain unchanged.
/// This initial operation is deliberately separate from canonical-blank TCNT.
pub struct AddTablePlan {
    before: NativeImage,
    after: NativeImageSource,
    definition: FirstTableDefinition,
    profile: AddTableSourceProfile,
}

/// Exact native source family retained by an append-table plan and its intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddTableSourceProfile {
    /// Original packed generated source/target construction.
    LegacyGenerated,
    /// Original mixed two/three-page packed ledgers.
    Mixed,
    /// Live and retired physical roles with fresh EOF allocation.
    Retained,
}

impl NativeImage {
    /// Append an empty manual or Increment table to a qualified bounded ledger.
    /// Each existing table retains one DATA page and its optional primary root.
    /// Imported catalogs, allocation expansion and broader schema changes refuse.
    pub fn plan_add_table(
        self,
        definition: FirstTableDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AddTablePlan, AddTableError> {
        if retained::selected(&self, budget, cancelled)? {
            return self.plan_add_table_retained(definition, budget, cancelled);
        }
        self.plan_add_table_mixed(definition, budget, cancelled)
    }

    /// Reconstruct the frozen TCAT5 mixed-source language without retained selection.
    pub(crate) fn plan_add_table_mixed(
        self,
        definition: FirstTableDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AddTablePlan, AddTableError> {
        let _span = tracing::debug_span!("native.add_table.plan").entered();
        check(budget, cancelled)?;
        let keyed = definition.primary_position().is_some();
        let (geometry, legacy) = source::derive_geometry(&self, keyed, budget, cancelled)?;
        if legacy && !definition.is_manual() {
            return self.plan_add_table_legacy(definition, budget, cancelled);
        }
        let original = source::qualify_geometry(&self, geometry, legacy, budget, cancelled)?;
        source::require_name(&self, definition.table(), budget, cancelled)?;
        let after = build::build(&self, &definition, &original, budget, cancelled)?;
        audit::verify(&self, &after, &definition, &original, budget, cancelled)?;
        check(budget, cancelled)?;
        let after = NativeImageSource::from_vec(after, budget, cancelled)?;
        Ok(AddTablePlan {
            before: self,
            after,
            definition,
            profile: AddTableSourceProfile::Mixed,
        })
    }

    /// Reconstruct only the TCAT6 retained append-source language.
    pub(crate) fn plan_add_table_retained(
        self,
        definition: FirstTableDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AddTablePlan, AddTableError> {
        retained::plan(self, definition, budget, cancelled)
    }

    /// Reconstruct only the original generated-source/generated-target language.
    /// Selected legacy recovery never gains mixed-source admission by fallback.
    pub(crate) fn plan_add_table_legacy(
        self,
        definition: FirstTableDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AddTablePlan, AddTableError> {
        check(budget, cancelled)?;
        if definition.is_manual() {
            return Err(CreateDatabaseError::Declaration.into());
        }
        let original = source::capture(&self, definition.table(), budget, cancelled)?;
        let after = build::build(&self, &definition, &original, budget, cancelled)?;
        audit::verify(&self, &after, &definition, &original, budget, cancelled)?;
        check(budget, cancelled)?;
        let after = NativeImageSource::from_vec(after, budget, cancelled)?;
        Ok(AddTablePlan {
            before: self,
            after,
            definition,
            profile: AddTableSourceProfile::LegacyGenerated,
        })
    }
}

impl AddTablePlan {
    /// Fresh structural source/target classification selects the closed wire language.
    pub const fn source_profile(&self) -> AddTableSourceProfile {
        self.profile
    }

    /// Complete retained original bytes; no existing object was reconstructed.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete independently checked append candidate, without a save capability.
    pub fn after_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }

    /// Exact proposed native table name.
    pub fn table(&self) -> &str {
        self.definition.table()
    }

    /// Number of explicit fields, including the requested Increment primary.
    pub fn column_count(&self) -> usize {
        self.definition.column_count()
    }

    pub(crate) fn definition(&self) -> &FirstTableDefinition {
        &self.definition
    }

    /// Discard the candidate while retaining its original owned declaration.
    pub fn into_definition(self) -> FirstTableDefinition {
        self.definition
    }
}

#[cfg(test)]
mod tests;
