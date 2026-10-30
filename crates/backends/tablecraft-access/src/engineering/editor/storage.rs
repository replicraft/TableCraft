//! Source-native storage selection; scalar Text/Binary alone grants no LVAL route.
use super::{AccessEngineeringEditor, NativeEditError, check};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

impl AccessEngineeringEditor {
    pub(super) fn ordinary_long_value_column(
        &self,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, NativeEditError> {
        let selected = self
            .snapshot
            .columns()
            .get(position)
            .ok_or(NativeEditError::Unsupported)?;
        let mut native = None;
        for column in self.native_schema.columns() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    selected.name().len().max(column.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if column.name().eq_ignore_ascii_case(selected.name())
                && native.replace(column).is_some()
            {
                return Err(InspectError::Malformed.into());
            }
        }
        let native = native.ok_or(NativeEditError::Unsupported)?;
        check(budget, cancelled)?;
        Ok(native.is_ordinary_long_value())
    }
}
