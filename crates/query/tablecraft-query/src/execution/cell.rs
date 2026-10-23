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
