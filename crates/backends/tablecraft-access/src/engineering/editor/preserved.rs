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
        if !self.projected_schema {
            return Ok(());
        }
        let schema = &self.native_schema;
        if !matches!(
            self.mode,
            ScalarUpdateMode::Default
                | ScalarUpdateMode::Fixed(UpdateMode::Fitted)
                | ScalarUpdateMode::Variable {
                    index_growth: false
                }
        ) {
            return Err(NativeEditError::Unsupported);
        }
        let target = self
            .snapshot
            .columns()
            .get(position)
            .ok_or(NativeEditError::Unsupported)?;
        let short_text = target.data_type() == ScalarType::Text;
        if matches!(self.mode, ScalarUpdateMode::Variable { .. }) && !short_text {
            return Err(NativeEditError::Unsupported);
        }
        if !matches!(
            target.data_type(),
            ScalarType::Boolean
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Currency
                | ScalarType::DateTime
        ) && !(short_text
            && matches!(
                self.mode,
                ScalarUpdateMode::Default
                    | ScalarUpdateMode::Variable {
                        index_growth: false
                    }
            ))
        {
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
        let native = native.ok_or(NativeEditError::Unsupported)?;
        if native.scalar_type() != Some(target.data_type())
            || (short_text && !native.is_ordinary_short_text())
        {
            return Err(NativeEditError::Unsupported);
        }
        Ok(())
    }
}
