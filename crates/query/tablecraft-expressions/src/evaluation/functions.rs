//! Only known unqualified scalar calls reach value evaluation.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{self, Builtin, text::TextFunction},
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
            (Some(Builtin::Nz), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                Ok(functions::nz_query(value, None))
            }
            (Some(Builtin::Nz), [A::Expression(value), A::Expression(replacement)]) => {
                let value = self.node(value, depth + 1)?;
                let replacement = self.node(replacement, depth + 1)?;
                Ok(functions::nz_query(value, Some(replacement)))
            }
            (Some(Builtin::Date(function)), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                function.apply(&value)
            }
            (Some(Builtin::Text(function)), arguments)
                if function.accepts_arity(arguments.len())
                    && arguments
                        .iter()
                        .all(|argument| matches!(argument, A::Expression(_))) =>
            {
                let mut values: [Value; TextFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    let A::Expression(expr) = argument else {
                        return Err(EvalError::UnsupportedExpression);
                    };
                    *value = self.node(expr, depth + 1)?;
                }
                self.apply_text_function(function, &values[..arguments.len()])
            }
            _ => Err(EvalError::UnsupportedExpression),
        }
    }
}
