//! Recognize only direct explicit array constructors, without evaluating them.
use super::ArrayValue;
use tablecraft_sql::select::{FunctionArgument, Name};

/// Whether this spelling is Array or the explicitly allowed VBA.Array form.
pub fn is_constructor(qualifier: &[Name], name: &Name) -> bool {
    name.as_str().eq_ignore_ascii_case("Array")
        && (qualifier.is_empty()
            || (qualifier.len() == 1 && qualifier[0].as_str().eq_ignore_ascii_case("VBA")))
}

/// Return supplied scalar element expressions within the structural ceiling.
/// Missing arguments and wildcards are rejected before any expression executes.
pub fn elements(arguments: &[FunctionArgument]) -> Option<&[FunctionArgument]> {
    (arguments.len() <= ArrayValue::MAX_ELEMENTS
        && arguments
            .iter()
            .all(|arg| matches!(arg, FunctionArgument::Expression(_))))
    .then_some(arguments)
}
