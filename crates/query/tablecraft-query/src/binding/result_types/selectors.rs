//! Selection facts refine successful carriers without removing eager expressions.
use super::{Binder, aggregate, literals};
use crate::{
    OutputType, QueryError,
    plan::{Aggregate, Expr, Slot},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError,
    functions::conditional::{ConditionalFunction, expression},
};
use tablecraft_sql::select::Expression;

#[derive(Clone, Copy)]
enum Mapping {
    Value,
    Aggregate(Aggregate),
}

#[derive(Clone, Copy)]
enum Index {
    Unknown,
    NoMatch,
    Choice(usize),
}

impl Binder<'_> {
    pub(super) fn iif_result_type(
        &mut self,
        condition: &Expr,
        yes: &Expr,
        no: &Expr,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        self.iif_mapped_type(Mapping::Value, condition, yes, no, scopes, depth)
    }

    fn iif_mapped_type(
        &mut self,
        mapping: Mapping,
        condition: &Expr,
        yes: &Expr,
        no: &Expr,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        // Inspect both result descriptions even when the selector is known.
        // This is metadata refinement, never dead-expression elimination.
        let yes = self.mapped_type(mapping, yes, scopes, depth)?;
        let no = self.mapped_type(mapping, no, scopes, depth)?;
        self.tick(depth)?;
        Ok(match literal_truth(condition)? {
            Some(true) => yes,
            Some(false) => no,
            None => yes.union(no),
        })
    }

    pub(super) fn selection_result_type(
        &mut self,
        function: ConditionalFunction,
        arguments: &[Expr],
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        self.selection_mapped_type(Mapping::Value, function, arguments, scopes, depth)
    }

    fn selection_mapped_type(
        &mut self,
        mapping: Mapping,
        function: ConditionalFunction,
        arguments: &[Expr],
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        match function {
            ConditionalFunction::Choose => {
                self.choose_result_type(mapping, arguments, scopes, depth)
            }
            ConditionalFunction::Switch => {
                self.switch_result_type(mapping, arguments, scopes, depth)
            }
        }
    }

    fn mapped_type(
        &mut self,
        mapping: Mapping,
        expression: &Expr,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        match mapping {
            Mapping::Value => self.result_type(expression, scopes, depth),
            Mapping::Aggregate(kind) => {
                self.numeric_aggregate_result_type(kind, expression, scopes, depth)
            }
        }
    }

    pub(super) fn numeric_aggregate_result_type(
        &mut self,
        kind: Aggregate,
        expression: &Expr,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        match expression {
            Expr::IIf(condition, yes, no) => {
                self.tick(depth)?;
                self.iif_mapped_type(
                    Mapping::Aggregate(kind),
                    condition,
                    yes,
                    no,
                    scopes,
                    depth + 1,
                )
            }
            Expr::ConditionalCall(function, arguments) => {
                self.tick(depth)?;
                self.selection_mapped_type(
                    Mapping::Aggregate(kind),
                    *function,
                    arguments,
                    scopes,
                    depth + 1,
                )
            }
            Expr::CVar(value) => {
                self.tick(depth)?;
                self.numeric_aggregate_result_type(kind, value, scopes, depth + 1)
            }
            _ => Ok(aggregate::numeric(
                kind,
                self.result_type(expression, scopes, depth)?,
            )),
        }
    }

    fn choose_result_type(
        &mut self,
        mapping: Mapping,
        arguments: &[Expr],
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        self.tick(depth)?;
        let index = match literal_value(arguments.first().ok_or(QueryError::Unsupported)?)? {
            Some(value) => {
                // Reuse Single admission, half-even rounding and one-based index
                // bounds. Unit payloads cannot allocate or execute providers.
                let mut selection =
                    ConditionalFunction::Choose.start_payload::<usize>(arguments.len())?;
                selection.push_control(ExpressionValue::Value(value))?;
                for index in 1..arguments.len() {
                    self.tick(depth)?;
                    selection.push_payload(index)?;
                }
                match selection.finish_payload() {
                    Ok(Some(index)) => Index::Choice(index),
                    Ok(None) => Index::NoMatch,
                    // A known invalid control proves no successful branch, but
                    // must not move a runtime semantic error into preparation.
                    Err(EvalError::Overflow | EvalError::NonFinite | EvalError::TypeMismatch) => {
                        Index::Unknown
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            None => Index::Unknown,
        };
        let mut result = OutputType::Null;
        for (position, argument) in arguments.iter().enumerate().skip(1) {
            let choice = self.mapped_type(mapping, argument, scopes, depth)?;
            match index {
                Index::Choice(index) if index == position => result = choice,
                Index::Unknown => result = result.union(choice),
                Index::Choice(_) | Index::NoMatch => {}
            }
        }
        Ok(result)
    }

    fn switch_result_type(
        &mut self,
        mapping: Mapping,
        arguments: &[Expr],
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<OutputType, QueryError> {
        let mut result = None;
        let mut fall_through = true;
        for pair in arguments.as_chunks::<2>().0 {
            self.tick(depth)?;
            let choice = self.mapped_type(mapping, &pair[1], scopes, depth)?;
            let truth = literal_truth(&pair[0])?;
            if fall_through && truth != Some(false) {
                result = Some(match result {
                    Some(previous) => OutputType::union(previous, choice),
                    None => choice,
                });
                if truth == Some(true) {
                    fall_through = false;
                }
            }
        }
        let result = result.unwrap_or(OutputType::Null);
        Ok(if fall_through {
            result.with_null()
        } else {
            result
        })
    }
}

fn literal_truth(expression: &Expr) -> Result<Option<bool>, QueryError> {
    let Some(value) = literal_value(expression)? else {
        return Ok(None);
    };
    match expression::iif(&ExpressionValue::Value(value), true, false) {
        Ok(value) => Ok(Some(value)),
        Err(EvalError::TypeMismatch | EvalError::NonFinite | EvalError::Overflow) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn literal_value(expression: &Expr) -> Result<Option<Value>, QueryError> {
    Ok(match expression {
        Expr::Literal(Expression::Null) => Some(Value::Null),
        Expr::Literal(Expression::Boolean(value)) => Some(Value::Boolean(*value)),
        Expr::Literal(Expression::Integer(value)) => Some(Value::Int64(*value)),
        Expr::Literal(value @ Expression::Number(_)) => Some(literals::checked(value)?),
        Expr::NumericLiteral(value) => Some(value.clone()),
        Expr::DateLiteral(_, value) => Some(Value::DateTime(*value)),
        // Text control conversion requires execution policies. Fields,
        // parameters, casts and other calls remain unknown without evaluation.
        _ => None,
    })
}
