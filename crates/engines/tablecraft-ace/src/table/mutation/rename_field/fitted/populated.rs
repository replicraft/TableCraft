//! A separate candidate for fully reconstructed nonempty row sources.
#[cfg(test)]
mod tests;
use super::{FittedFieldRenameSource, Prepared, layout::Layout, prepare};
use crate::{
    NativeFieldRenameIdentity, NativeImage,
    header::check,
    table::mutation::{
        rename_field::{FieldRenameError as Error, populated_source},
        saved_field::Observed,
    },
};
use tablecraft_core::limits::ResourceBudget;

/// Sealed metadata-only rename of a populated, replay-qualified source.
/// Its nonempty source family is distinct from both existing empty profiles.
pub struct NativePopulatedFieldRenameCandidate {
    prepared: Prepared,
}

impl NativePopulatedFieldRenameCandidate {
    /// Complete source including every original row, key and counter byte.
    pub fn before_bytes(&self) -> &[u8] {
        self.prepared.source.image().as_bytes()
    }

    /// Complete replacement; only the qualified metadata name frames change.
    pub fn after_bytes(&self) -> &[u8] {
        &self.prepared.after
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.prepared.source.observed.schema.name()
    }

    /// Actual original field spelling.
    pub fn column(&self) -> &str {
        self.prepared.source.observed.column().name()
    }

    /// Exact requested ASCII spelling, including unchanged and case-only names.
    pub fn new_name(&self) -> &str {
        &self.prepared.new_name
    }

    /// Native identity, distinct from a visible column position.
    pub const fn identity(&self) -> NativeFieldRenameIdentity {
        NativeFieldRenameIdentity::from_observed(self.prepared.source.observed.identity)
    }

    /// Exact spelling change after complete source and request validation.
    pub fn changed(&self) -> bool {
        self.column() != self.new_name()
    }
}

impl NativeImage {
    /// Inspect a genuinely populated source without fabricating a proposed name.
    pub fn inspect_populated_field_rename_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FittedFieldRenameSource, Error> {
        populated_source::validate(self, table, budget, cancelled)?;
        let observed = Observed::capture(self, table, column, budget, cancelled)?;
        if observed.identity.column_number() != 1 {
            return Err(Error::Source);
        }
        let layout = Layout::capture(self, &observed, budget, cancelled)?;
        Ok(FittedFieldRenameSource {
            identity: NativeFieldRenameIdentity::from_observed(observed.identity),
            maximum_name_units: layout.maximum_name_units,
        })
    }

    /// Rename a replay-qualified populated field at any fitting ASCII width.
    /// Equal-width, case-only and no-op requests retain this same source family.
    pub fn prepare_populated_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativePopulatedFieldRenameCandidate, Error> {
        let _span = tracing::debug_span!("native.field_rename.prepare_populated").entered();
        check(budget, cancelled)?;
        if !crate::identifier::valid_name(new_name, budget, cancelled)? {
            return Err(Error::InvalidName);
        }
        if !new_name.is_ascii() {
            return Err(Error::PopulatedNameShape);
        }
        let source = populated_source::capture(self, table, column, budget, cancelled)?;
        let prepared = prepare(source, new_name, budget, cancelled)?;
        Ok(NativePopulatedFieldRenameCandidate { prepared })
    }
}
