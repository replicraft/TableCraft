//! Retained native declarations constrain the projected engineering snapshot.
use super::{AccessEngineeringEditor, NativeEditError, ScalarUpdateMode, UpdateMode};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

impl AccessEngineeringEditor {
    pub(super) fn check_preserved_target(
        &self,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeEditError> {
        let Some(schema) = &self.preserved_schema else {
            return Ok(());
        };
        if !matches!(
            self.mode,
            ScalarUpdateMode::Default | ScalarUpdateMode::Fixed(UpdateMode::Fitted)
        ) {
            return Err(NativeEditError::Unsupported);
        }
        let target = self
            .snapshot
            .columns()
            .get(position)
            .ok_or(NativeEditError::Unsupported)?;
        if target.data_type() != ScalarType::Boolean {
            return Err(NativeEditError::Unsupported);
        }
        let mut native = None;
        for field in schema.columns() {
            if cancelled() {
                return Err(InspectError::Cancelled.into());
            }
            budget
                .charge(
                    Resource::Steps,
                    field.name().len().max(target.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if field.name().eq_ignore_ascii_case(target.name()) && native.replace(field).is_some() {
                return Err(NativeEditError::Unsupported);
            }
        }
        if native.and_then(|field| field.scalar_type()) != Some(ScalarType::Boolean) {
            return Err(NativeEditError::Unsupported);
        }
        Ok(())
    }
}
