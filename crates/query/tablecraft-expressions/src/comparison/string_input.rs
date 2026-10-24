//! Borrowed String inputs shared by raw comparison and pattern matching.
use crate::EvalError;
use tablecraft_core::limits::{Resource, ResourceBudget};

const UNIT_BYTES: usize = size_of::<u16>();

/// A non-NULL String supplied without Unicode publication.
#[derive(Clone, Copy, Debug)]
pub enum StringInput<'a> {
    /// Already decoded Unicode text.
    Text(&'a str),
    /// Little-endian String bytes. Complete units may contain isolated surrogates;
    /// odd byte counts are refused rather than padded or truncated.
    Utf16Le(&'a [u8]),
}

impl StringInput<'_> {
    pub(crate) fn units(
        self,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Vec<u16>, EvalError> {
        check(budget, cancelled)?;
        match self {
            Self::Text(text) => {
                let mut units = buffer(text.len(), budget)?;
                for unit in text.encode_utf16() {
                    check(budget, cancelled)?;
                    units.push(unit);
                }
                Ok(units)
            }
            Self::Utf16Le(bytes) => {
                i32::try_from(bytes.len()).map_err(|_| EvalError::Overflow)?;
                let (chunks, remainder) = bytes.as_chunks::<UNIT_BYTES>();
                if !remainder.is_empty() {
                    return Err(EvalError::UnrepresentableText);
                }

                let mut units = buffer(chunks.len(), budget)?;
                for chunk in chunks {
                    check(budget, cancelled)?;
                    units.push(u16::from_le_bytes(*chunk));
                }
                Ok(units)
            }
        }
    }
}

fn buffer(count: usize, budget: &mut ResourceBudget) -> Result<Vec<u16>, EvalError> {
    budget.charge_allocation(count as u64, UNIT_BYTES as u64)?;
    let mut units = Vec::new();
    units
        .try_reserve_exact(count)
        .map_err(|_| EvalError::Allocation)?;
    Ok(units)
}

pub(super) fn check(
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), EvalError> {
    if cancelled.is_some_and(|cancelled| cancelled()) {
        return Err(EvalError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
