//! Only known unqualified scalar calls reach value evaluation.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{self, Builtin},
};
use tablecraft_core::values::Value;
use tablecraft_sql::select::{FunctionArgument as A, Name};

impl Evaluator<'_> {
    pub(super) fn call(
        &mut self,
        qualifier: &[Name],
        name: &Name,
        arguments: &[A],
        depth: usize,
    ) -> Result<Value, EvalError> {
        if !qualifier.is_empty() {
            return Err(EvalError::UnsupportedExpression);
        }
        match (Builtin::resolve(name.as_str()), arguments) {
            (Some(Builtin::IsNull), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                Ok(Value::Boolean(functions::is_null(&value)))
            }
            (
                Some(Builtin::IIf),
                [
                    A::Expression(condition),
                    A::Expression(truepart),
                    A::Expression(falsepart),
                ],
            ) => {
                let condition = self.node(condition, depth + 1)?;
                let truepart = self.node(truepart, depth + 1)?;
                let falsepart = self.node(falsepart, depth + 1)?;
                functions::iif(&condition, truepart, falsepart)
            }
            _ => Err(EvalError::UnsupportedExpression),
        }
    }
}
