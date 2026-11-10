//! Real captured original plans; target declarations are derived, never echoed.
use std::num::NonZeroU8;
use tablecraft_ace::{
    FieldAdditionTarget, RecoverableCurrentFieldAdditionPlan, RecoverableDefaultZeroAddColumnPlan,
    RecoverableFieldAdditionAfterOffHistoryPlan, RecoverableFieldAdditionAfterOffPlan,
    RecoverableFieldAdditionPlan, RecoverableRepeatedDefaultZeroAddColumnPlan,
};
use tablecraft_builders::table::FieldAdditionDeclaration;

pub(in crate::editing) enum Plan {
    First(RecoverableDefaultZeroAddColumnPlan),
    Repeated(RecoverableRepeatedDefaultZeroAddColumnPlan),
    Typed(RecoverableFieldAdditionPlan),
    AfterOff(RecoverableFieldAdditionAfterOffPlan),
    AfterOffHistory(RecoverableFieldAdditionAfterOffHistoryPlan),
    Current(Box<RecoverableCurrentFieldAdditionPlan>),
}

impl Plan {
    pub(in crate::editing) fn captured_declaration(
        &self,
    ) -> Result<FieldAdditionDeclaration, crate::Error> {
        Ok(match self {
            Self::First(plan) => {
                if plan.required_property()
                    != tablecraft_ace::CreatedDefaultZeroColumnRequired::StoredFalse
                {
                    return Err(crate::editing::EditError::InvalidInput.into());
                }
                FieldAdditionDeclaration::NumberLongZero
            }
            Self::Repeated(_) | Self::Current(_) => FieldAdditionDeclaration::NumberLongZero,
            Self::Typed(plan) => captured_target(plan.target())?,
            Self::AfterOff(plan) => captured_target(plan.target())?,
            Self::AfterOffHistory(plan) => captured_target(plan.target())?,
        })
    }

    pub(in crate::editing) fn table(&self) -> &str {
        match self {
            Self::First(plan) => plan.table(),
            Self::Repeated(plan) => plan.table(),
            Self::Typed(plan) => plan.table(),
            Self::AfterOff(plan) => plan.table(),
            Self::AfterOffHistory(plan) => plan.table(),
            Self::Current(plan) => plan.table(),
        }
    }

    pub(in crate::editing) fn column(&self) -> &str {
        match self {
            Self::First(plan) => plan.column(),
            Self::Repeated(plan) => plan.column(),
            Self::Typed(plan) => plan.column(),
            Self::AfterOff(plan) => plan.column(),
            Self::AfterOffHistory(plan) => plan.column(),
            Self::Current(plan) => plan.column(),
        }
    }

    pub(in crate::editing) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::First(plan) => plan.before_bytes(),
            Self::Repeated(plan) => plan.before_bytes(),
            Self::Typed(plan) => plan.before_bytes(),
            Self::AfterOff(plan) => plan.before_bytes(),
            Self::AfterOffHistory(plan) => plan.before_bytes(),
            Self::Current(plan) => plan.before_bytes(),
        }
    }

    pub(in crate::editing) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::First(plan) => plan.after_bytes(),
            Self::Repeated(plan) => plan.after_bytes(),
            Self::Typed(plan) => plan.after_bytes(),
            Self::AfterOff(plan) => plan.after_bytes(),
            Self::AfterOffHistory(plan) => plan.after_bytes(),
            Self::Current(plan) => plan.after_bytes(),
        }
    }
}

fn captured_target(target: FieldAdditionTarget) -> Result<FieldAdditionDeclaration, crate::Error> {
    Ok(match target {
        FieldAdditionTarget::NumberLongZero => FieldAdditionDeclaration::NumberLongZero,
        FieldAdditionTarget::ShortText {
            max_utf16_units,
            unicode_compression,
        } => {
            let width = u8::try_from(max_utf16_units)
                .ok()
                .and_then(NonZeroU8::new)
                .ok_or(crate::editing::EditError::InvalidInput)?;
            FieldAdditionDeclaration::ShortText {
                max_utf16_units: width,
                unicode_compression,
            }
        }
    })
}
