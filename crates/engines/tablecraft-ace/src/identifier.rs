//! Native identifier policy shared by actual creation and rename operations.
use crate::{InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

const MAX_NAME_UNITS: usize = 64;

/// Borrowed native spelling validation. Callers retain operation-specific errors
/// and their original leading cancellation/guard order.
pub(crate) fn valid_name(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    if name.is_empty() || name.starts_with(' ') {
        return Ok(false);
    }
    let mut units = 0;
    for character in name.chars() {
        check(budget, cancelled)?;
        units += character.len_utf16();
        if character.is_control()
            || matches!(character, '.' | '!' | '`' | '[' | ']')
            || units > MAX_NAME_UNITS
        {
            return Ok(false);
        }
    }
    Ok(true)
}
