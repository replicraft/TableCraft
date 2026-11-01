//! Positive-program admission over independent native checked index facts.
use crate::{
    EditError, NativeImage, TableSchema, table::mutation::fixed_records::indexed::CheckedIndexes,
};
#[cfg(feature = "journal")]
use crate::{
    TableCheck,
    table::mutation::{guards::native_events, normal_before_change::event_error},
};
use tablecraft_core::limits::ResourceBudget;

/// Positive-program wrapper. Native facts alone do not grant event admission,
/// ordinary content closure, indexed mutation or a durable write capability.
pub(in crate::table::mutation) struct CheckedTargets<'image> {
    facts: CheckedIndexes<'image>,
}

impl<'image> CheckedTargets<'image> {
    #[cfg(feature = "journal")]
    pub(in crate::table::mutation) fn capture_native(
        image: &'image NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let facts = CheckedIndexes::capture(image, table, budget, cancelled)?;
        let schema = facts.checked().schema();
        // There is no caller selector list or absence boolean in this entry.
        let programs =
            native_events::programs(image, schema, budget, cancelled).map_err(event_error)?;
        programs.require_source(image, schema, budget, cancelled)?;
        Ok(Self { facts })
    }

    #[cfg(feature = "journal")]
    pub(in crate::table::mutation) fn checked(&self) -> &TableCheck {
        self.facts.checked()
    }

    pub(in crate::table::mutation) fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.facts.require_source(image, schema, budget, cancelled)
    }

    pub(in crate::table::mutation) fn require_target(
        &self,
        schema: &TableSchema,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.facts
            .require_target(schema, position, budget, cancelled)
    }
}
