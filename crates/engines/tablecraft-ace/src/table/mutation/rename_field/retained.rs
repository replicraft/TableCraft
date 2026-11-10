//! Retained current metadata appends a snapshot without moving prior property rows.
mod build;
mod source;
mod verify;

#[cfg(test)]
mod tests;

use super::{FieldRenameError as Error, FittedFieldRenameSource, NativeFieldRenameIdentity};
use crate::{
    NativeImage,
    header::check,
    table::mutation::{catalog_property, normal_fitted::binding},
};
use tablecraft_core::limits::ResourceBudget;

/// Sealed fitting rename over checked current storage and retained snapshots.
/// This candidate cannot be constructed from a decoded request or a raw image.
pub struct NativeRetainedFieldRenameCandidate {
    source: source::Source,
    after: Vec<u8>,
    new_name: String,
}

impl NativeRetainedFieldRenameCandidate {
    /// Complete source-qualified original, including old property snapshots.
    pub fn before_bytes(&self) -> &[u8] {
        self.source.image.as_bytes()
    }

    /// Complete replacement, retaining every original DATA and INDEX byte.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Original native table spelling.
    pub fn table(&self) -> &str {
        self.source.observed.schema.name()
    }

    /// Original native field spelling, independent of its visible position.
    pub fn column(&self) -> &str {
        self.source.observed.column().name()
    }

    /// Exact requested ASCII spelling, including case-only and no-op names.
    pub fn new_name(&self) -> &str {
        &self.new_name
    }

    /// Original logical number and independently stored creation-time ID.
    pub const fn identity(&self) -> NativeFieldRenameIdentity {
        NativeFieldRenameIdentity::from_observed(self.source.observed.identity)
    }

    /// Whether the exact spelling changes after complete source validation.
    pub fn changed(&self) -> bool {
        self.column() != self.new_name()
    }
}

impl NativeImage {
    /// Metadata chooses this distinct request language before source admission.
    pub fn select_native_retained_field_rename_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, Error> {
        source::selected(self, table, column, budget, cancelled)
    }

    /// Current ownership and exact snapshot capacity, without a proposed name.
    pub fn inspect_retained_field_rename_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FittedFieldRenameSource, Error> {
        let (observed, maximum_name_units) =
            source::observe(self, table, column, budget, cancelled)?;
        Ok(FittedFieldRenameSource {
            identity: NativeFieldRenameIdentity::from_observed(observed.identity),
            maximum_name_units,
        })
    }

    /// Rename only the native name and its live property group, retaining history.
    pub fn prepare_retained_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeRetainedFieldRenameCandidate, Error> {
        check(budget, cancelled)?;
        if !crate::identifier::valid_name(new_name, budget, cancelled)? {
            return Err(Error::InvalidName);
        }
        if !new_name.is_ascii() {
            return Err(Error::CurrentNameShape);
        }

        let source = source::Source::capture(self, table, column, budget, cancelled)?;
        source::request(&source, new_name, budget, cancelled)?;

        let mut after = build::definition(&source, new_name, budget, cancelled)?;
        if source.observed.column().name() != new_name {
            let body = build::properties(&source.observed, new_name, budget, cancelled)?;
            let change = catalog_property::compose_retained_single_at_origin(
                &source.image,
                &source.observed.schema,
                body,
                budget,
                cancelled,
            )?;
            change.apply(&mut after, budget, cancelled)?;
            verify::candidate(&source, &after, new_name, &change, budget, cancelled)?;
        } else if !binding::equal_bytes(source.image.as_bytes(), &after, budget, cancelled)? {
            return Err(Error::Preservation);
        }

        let new_name = build::owned_name(new_name, budget)?;
        Ok(NativeRetainedFieldRenameCandidate {
            source,
            after,
            new_name,
        })
    }
}
