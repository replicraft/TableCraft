//! Positive native current-source and UPDATE program/constraint closure.
use super::super::{
    current_schema, no_required_programs::NoRequiredTablePrograms, preserved_source,
};
use crate::{EditError, NativeImage, WriteBlocker};
use tablecraft_core::limits::ResourceBudget;

/// This proof has a separate purpose from ordinary and readonly schema sources.
/// Each consumer independently qualifies its present or absent field requests.
pub(in crate::table::mutation) struct Source<'image> {
    image: &'image NativeImage,
    pub(super) facts: preserved_source::Source,
}

impl<'image> Source<'image> {
    pub(in crate::table::mutation) fn capture(
        image: &'image NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let current = current_schema::observe(image, table, budget, cancelled).map_err(
            |error| match error {
                current_schema::Error::Inspect(error) => EditError::Inspect(error),
                current_schema::Error::Native(error) => error,
                current_schema::Error::Source => EditError::Unsupported(WriteBlocker::RowLayout),
            },
        )?;
        let certificate = NoRequiredTablePrograms::capture_historical_fixed_rows(
            image, &current, budget, cancelled,
        )?;
        let facts = preserved_source::capture_historical_fixed_rows(
            image,
            table,
            &certificate,
            budget,
            cancelled,
        )?;
        if !facts.dependencies.is_empty() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        Ok(Self { image, facts })
    }

    pub(in crate::table::mutation) fn facts(&self) -> &preserved_source::Source {
        &self.facts
    }

    pub(in crate::table::mutation) fn require_image(
        &self,
        image: &NativeImage,
    ) -> Result<(), EditError> {
        if !std::ptr::eq(self.image, image) {
            return Err(EditError::Preservation);
        }
        Ok(())
    }
}
