//! Execution-local text provenance, separate from stored and public expression values.
use crate::source::TextPolicy;
use std::ops::Deref;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError, functions::conversions::errors, numeric::OperandDeclaration,
};

pub(crate) struct CellValue {
    pub(super) value: ExpressionValue,
    pub(super) policy: TextPolicy,
    // Constructed String lineage is already complete; static metadata may be
    // conservative because an operand could also have returned another carrier.
    pub(super) policy_complete: bool,
    pub(super) declaration: Option<OperandDeclaration>,
}
impl CellValue {
    pub(super) fn scalar(value: Value) -> Self {
        Self::from(ExpressionValue::Value(value))
    }
    pub(super) fn with_declaration(mut self, declaration: Option<OperandDeclaration>) -> Self {
        self.declaration = declaration;
        self
    }
    pub(super) fn with_complete_policy(mut self, complete: bool) -> Self {
        self.policy_complete = complete;
        self
    }
    pub(super) fn with_policy(mut self, policy: TextPolicy) -> Self {
        self.policy = self.policy.merge(policy);
        self
    }
}
impl From<ExpressionValue> for CellValue {
    fn from(value: ExpressionValue) -> Self {
        Self {
            value,
            policy: TextPolicy::Default,
            policy_complete: false,
            declaration: None,
        }
    }
}
impl Deref for CellValue {
    type Target = ExpressionValue;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
pub(super) fn into_scalar(value: CellValue) -> Result<Value, EvalError> {
    errors::into_scalar(value.value)
}
pub(super) fn into_operator_scalar(value: CellValue) -> Result<Value, EvalError> {
    errors::into_operator_scalar(value.value)
}

pub(crate) type CellQueryResult = super::QueryResult<CellValue>;

/// Complete runtime cells already contain their contributing field policies.
/// Other cells retain static fallback, never replacing an actual peer policy.
pub(super) fn comparison_policy(
    left: &CellValue,
    right: &CellValue,
    fallback: TextPolicy,
) -> TextPolicy {
    let policy = left.policy.merge(right.policy);
    if left.policy_complete || right.policy_complete {
        policy
    } else {
        fallback.merge(policy)
    }
}
