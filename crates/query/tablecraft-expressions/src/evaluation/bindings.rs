use crate::EvalError;
use tablecraft_core::values::Value;
use tablecraft_sql::select::FieldReference;

/// Host-supplied borrowed scalar bindings, without opening a database.
///
/// The host owns name/parameter resolution and callback work. Missing bindings
/// must be errors, never silently fabricated NULLs.
pub trait Bindings {
    /// Resolve one written field/parameter, preserving its qualifier.
    fn resolve(&self, field: &FieldReference) -> Result<&Value, EvalError>;
}
/// A constant-only environment that rejects every field reference.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoBindings;
impl Bindings for NoBindings {
    fn resolve(&self, _: &FieldReference) -> Result<&Value, EvalError> {
        Err(EvalError::UnboundField)
    }
}
