//! InStr alone accepts explicit LCIDs; other functions keep the closed mode set.
use super::super::compare;
use crate::{
    EvalError,
    comparison::{Collation, LocaleId, StringCompareMode},
};
use tablecraft_core::values::Value;

const LAST_STANDARD_MODE: i32 = 2;

pub(super) enum SearchMode {
    Compare(Option<StringCompareMode>),
    Locale(LocaleId),
}
impl SearchMode {
    pub(super) fn resolve(self, collation: Option<&dyn Collation>) -> Result<Self, EvalError> {
        Ok(match self {
            Self::Compare(mode) => Self::Compare(Some(
                mode.or_else(|| collation.and_then(Collation::option_compare))
                    .ok_or(EvalError::CollationRequired)?,
            )),
            Self::Locale(locale) => Self::Locale(locale),
        })
    }
}
pub(super) fn forward(argument: Option<&Value>) -> Result<SearchMode, EvalError> {
    let code = compare::code(argument)?;
    match code {
        Some(code) if code > LAST_STANDARD_MODE => {
            Ok(SearchMode::Locale(LocaleId::new(code as u32)?))
        }
        _ => Ok(SearchMode::Compare(compare::from_code(code, None)?)),
    }
}
