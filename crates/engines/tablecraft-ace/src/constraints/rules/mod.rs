mod bindings;
mod compile;

use super::{ConstraintViolation, RuleCheckStatus};
use crate::{InspectError, PropertyGroup, TableSnapshot, header::check};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator},
};

pub(super) fn audit(
    table: &TableSnapshot,
    group: Option<&PropertyGroup>,
    field: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RuleCheckStatus, InspectError> {
    let Some(property) = group.and_then(|group| group.get("ValidationRule")) else {
        return Ok(RuleCheckStatus::NotDeclared);
    };
    let Ok(text) = property.text() else {
        return Ok(RuleCheckStatus::Unsupported);
    };
    if text.trim().is_empty() {
        return Ok(RuleCheckStatus::NotDeclared);
    }
    let Some(expression) = compile::rule(table, text, field, budget, cancelled)? else {
        return Ok(RuleCheckStatus::Unsupported);
    };
    evaluate_rows(table, &expression, field, budget, cancelled)
}

fn evaluate_rows(
    table: &TableSnapshot,
    expression: &tablecraft_sql::select::Expression,
    field: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RuleCheckStatus, InspectError> {
    let mut unknown = false;
    for row in table.rows() {
        check(budget, cancelled)?;
        let bindings = bindings::RowBindings {
            columns: table.columns(),
            row,
            table: table.schema().name(),
            field,
        };
        let value = Evaluator::new(&bindings, budget, EvalLimits::default())
            .with_cancellation(cancelled)
            .evaluate(expression)
            .map_err(evaluation_error)?;
        match value {
            Value::Boolean(true) => {}
            Value::Boolean(false) => {
                return Err(if field.is_some() {
                    ConstraintViolation::FieldRule
                } else {
                    ConstraintViolation::TableRule
                }
                .into());
            }
            // NULL validation results need actual Access comparison before this
            // audit can qualify them. They are neither accepted nor rejected.
            Value::Null => unknown = true,
            _ => return Ok(RuleCheckStatus::Unsupported),
        }
    }
    check(budget, cancelled)?;
    Ok(if unknown {
        RuleCheckStatus::Unsupported
    } else {
        RuleCheckStatus::Checked
    })
}

fn evaluation_error(error: EvalError) -> InspectError {
    match error {
        EvalError::Cancelled => InspectError::Cancelled,
        EvalError::Limit(error) => InspectError::Limit(error),
        EvalError::Allocation => InspectError::Allocation,
        EvalError::DepthLimit | EvalError::TextLimit => InspectError::StructureLimit,
        error => InspectError::Constraint(ConstraintViolation::Evaluation(error)),
    }
}

#[cfg(test)]
mod tests;
