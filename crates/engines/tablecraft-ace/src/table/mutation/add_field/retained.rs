//! Explicit retained ADD entry points; current source constructors remain closed.
use super::{
    CandidateParts, CurrentFieldAdditionError, CurrentFieldAdditionSource,
    NativeCurrentFieldAdditionCandidate, NativeCurrentShortTextFieldAdditionCandidate, Target,
    definition::Plan, facts, prepare,
};
use crate::{NativeImage, table::mutation::current_schema::Family};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Inspect retained ownership and actual fitting Number name capacity.
    pub fn inspect_retained_field_addition_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CurrentFieldAdditionSource, CurrentFieldAdditionError> {
        let current = Family::Retained.observe(self, table, budget, cancelled)?;
        let plan = Plan::capture(self, &current.schema, budget, cancelled)?;
        facts(self, &current, &plan, Target::Number, budget, cancelled)
    }

    /// Append nullable LONG with Required(false) and literal zero default.
    /// Historical records retain their bytes and expose the new field as NULL.
    pub fn prepare_retained_number_field_addition(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentFieldAdditionCandidate, CurrentFieldAdditionError> {
        let _span = tracing::debug_span!("native.field_addition.prepare_retained_number").entered();
        let CandidateParts {
            proof,
            after,
            column,
            facts,
        } = prepare(
            self,
            table,
            column,
            Target::Number,
            Family::Retained,
            budget,
            cancelled,
        )?;
        Ok(NativeCurrentFieldAdditionCandidate {
            proof,
            after,
            column,
            facts,
        })
    }

    /// Inspect retained ownership and actual fitting Short Text name capacity.
    pub fn inspect_retained_short_text_field_addition_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CurrentFieldAdditionSource, CurrentFieldAdditionError> {
        let current = Family::Retained.observe(self, table, budget, cancelled)?;
        let plan = Plan::capture(self, &current.schema, budget, cancelled)?;
        facts(
            self,
            &current,
            &plan,
            Target::ShortText {
                max_utf16_units: 255,
                unicode_compression: true,
            },
            budget,
            cancelled,
        )
    }

    /// Append nullable Short Text with Required(false), AllowZeroLength(true)
    /// and absent DefaultValue, preserving every historical record.
    pub fn prepare_retained_short_text_field_addition(
        self,
        table: &str,
        column: &str,
        max_utf16_units: usize,
        unicode_compression: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentShortTextFieldAdditionCandidate, CurrentFieldAdditionError> {
        let _span =
            tracing::debug_span!("native.field_addition.prepare_retained_short_text").entered();
        if !(1..=crate::format::data_type::MAX_SHORT_TEXT_UNITS).contains(&max_utf16_units) {
            return Err(CurrentFieldAdditionError::Source);
        }
        let target = Target::ShortText {
            max_utf16_units,
            unicode_compression,
        };
        let CandidateParts {
            proof,
            after,
            column,
            facts,
        } = prepare(
            self,
            table,
            column,
            target,
            Family::Retained,
            budget,
            cancelled,
        )?;
        Ok(NativeCurrentShortTextFieldAdditionCandidate {
            proof,
            after,
            column,
            facts,
            max_utf16_units,
            unicode_compression,
        })
    }
}

#[cfg(test)]
mod tests;
