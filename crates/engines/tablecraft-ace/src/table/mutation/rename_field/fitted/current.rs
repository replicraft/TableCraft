//! Rename current metadata without inferring creator or row-edit history.
#[cfg(test)]
mod tests;
use super::{FittedFieldRenameSource, Prepared, layout::Layout, prepare};
use crate::{
    NativeFieldRenameIdentity, NativeImage,
    header::check,
    table::mutation::rename_field::{FieldRenameError as Error, current_source},
};
use tablecraft_core::limits::ResourceBudget;

/// Sealed field rename over checked current ownership, metadata and dependencies.
/// Empty, populated and retained row/name histories use the same source proof.
pub struct NativeCurrentFieldRenameCandidate {
    prepared: Prepared,
}

impl NativeCurrentFieldRenameCandidate {
    /// Complete immutable original, including opaque free and historical bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.prepared.source.image().as_bytes()
    }

    /// Complete replacement with only parser-bound metadata packing changes.
    pub fn after_bytes(&self) -> &[u8] {
        &self.prepared.after
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.prepared.source.observed.schema.name()
    }

    /// Actual current field spelling, independent of historical property names.
    pub fn column(&self) -> &str {
        self.prepared.source.observed.column().name()
    }

    /// Exact requested ASCII spelling, including unchanged and case-only names.
    pub fn new_name(&self) -> &str {
        &self.prepared.new_name
    }

    /// Original native field number and stored ID, never a display position.
    pub const fn identity(&self) -> NativeFieldRenameIdentity {
        NativeFieldRenameIdentity::from_observed(self.prepared.source.observed.identity)
    }

    /// Exact spelling change after complete source and request validation.
    pub fn changed(&self) -> bool {
        self.column() != self.new_name()
    }
}

impl NativeImage {
    /// Qualify current metadata and fitting capacity without a proposed name.
    /// This observer neither rewrites history nor grants journal authority.
    pub fn inspect_current_field_rename_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FittedFieldRenameSource, Error> {
        let observed = current_source::observe(self, table, column, budget, cancelled)?;
        let layout = Layout::capture_current(self, &observed, budget, cancelled)?;
        Ok(FittedFieldRenameSource {
            identity: NativeFieldRenameIdentity::from_observed(observed.identity),
            maximum_name_units: layout.maximum_name_units,
        })
    }

    /// Rename a field through its actual current ownership and dependency proof.
    /// All fitting ASCII widths, no-ops and previous rename histories share this
    /// one family; legacy candidates retain their existing source languages.
    pub fn prepare_current_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentFieldRenameCandidate, Error> {
        let _span = tracing::debug_span!("native.field_rename.prepare_current").entered();
        check(budget, cancelled)?;
        if !crate::identifier::valid_name(new_name, budget, cancelled)? {
            return Err(Error::InvalidName);
        }
        if !new_name.is_ascii() {
            return Err(Error::CurrentNameShape);
        }
        let source = current_source::capture(self, table, column, budget, cancelled)?;
        let prepared = prepare(source, new_name, budget, cancelled)?;
        Ok(NativeCurrentFieldRenameCandidate { prepared })
    }
}
