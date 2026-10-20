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
            (
                Some(Builtin::DateAdd),
                [
                    A::Expression(interval),
                    A::Expression(number),
                    A::Expression(date),
                ],
            ) => {
                let interval = self.node(interval, depth + 1)?;
                let number = self.node(number, depth + 1)?;
                let date = self.node(date, depth + 1)?;
                functions::dates::date_add(&interval, &number, &date)
            }
            (
                Some(Builtin::DateDiff),
                [
                    A::Expression(interval),
                    A::Expression(first),
                    A::Expression(second),
                    options @ ..,
                ],
            ) if options.len() <= 2
                && options
                    .iter()
                    .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
            {
                let interval = self.node(interval, depth + 1)?;
                let first = self.node(first, depth + 1)?;
                let second = self.node(second, depth + 1)?;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let A::Expression(expr) = argument {
                        *setting = Some(self.node(expr, depth + 1)?);
                    }
                }
                functions::dates::date_diff(
                    &interval,
                    &first,
                    &second,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                )
            }
            (
                Some(Builtin::DatePart),
                [A::Expression(interval), A::Expression(date), options @ ..],
            ) if options.len() <= 2
                && options
                    .iter()
                    .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
            {
                let interval = self.node(interval, depth + 1)?;
                let date = self.node(date, depth + 1)?;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let A::Expression(expr) = argument {
                        *setting = Some(self.node(expr, depth + 1)?);
                    }
                }
                functions::dates::date_part(
                    &interval,
                    &date,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                )
            }
            (Some(Builtin::Convert(conversion)), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                conversion.apply(&value)
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
