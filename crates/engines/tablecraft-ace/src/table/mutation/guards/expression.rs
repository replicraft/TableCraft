use super::{BeforeEvent, BeforeEventGuardError, BeforeGuardUnsupported};
use crate::{InspectError, TableSnapshot, header::check};
use std::fmt::Write;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    ParseErrorKind, ParseLimits, parse_expression,
    select::{Expression, FieldReference, FunctionArgument, Name},
};

pub(super) const CURRENT: &str = "__guard_current";
pub(super) const OLD: &str = "__guard_old";
pub(super) const UPDATED: &str = "__guard_updated";
pub(super) const STATE: &str = "__guard_state";

const MAX_BYTES: usize = 4096;
const MAX_TOKENS: usize = 128;
const MAX_DEPTH: usize = 16;

pub(super) fn compile(
    text: &str,
    event: BeforeEvent,
    table: &TableSnapshot,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Expression, BeforeEventGuardError> {
    check(budget, cancelled)?;
    if text.len() > MAX_BYTES {
        return Err(InspectError::StructureLimit.into());
    }
    budget
        .charge(Resource::Steps, text.len() as u64)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(MAX_TOKENS as u64, size_of::<Expression>() as u64)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(text.len() as u64, 2)
        .map_err(InspectError::from)?;
    let mut expression = parse_expression(text, limits()).map_err(|error| {
        if error.kind == ParseErrorKind::Limit {
            BeforeEventGuardError::Inspect(InspectError::StructureLimit)
        } else {
            BeforeEventGuardError::Expression
        }
    })?;
    lower(&mut expression, event, table, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(expression)
}

fn limits() -> ParseLimits {
    ParseLimits {
        max_bytes: MAX_BYTES,
        max_tokens: MAX_TOKENS,
        max_depth: MAX_DEPTH,
    }
}

fn position(
    name: &str,
    table: &TableSnapshot,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, BeforeEventGuardError> {
    for (position, column) in table.columns().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (column.name().len() + name.len()) as u64)
            .map_err(InspectError::from)?;
        if column.name().eq_ignore_ascii_case(name) {
            return Ok(position);
        }
    }
    Err(BeforeEventGuardError::Unsupported(
        BeforeGuardUnsupported::Context,
    ))
}

fn binding(
    space: &'static str,
    position: usize,
    budget: &mut ResourceBudget,
) -> Result<Expression, BeforeEventGuardError> {
    // Twenty decimal digits cover every usize, including a 64-bit target.
    budget
        .charge_allocation(20 + space.len() as u64, 2)
        .map_err(InspectError::from)?;
    let mut digits = String::new();
    digits
        .try_reserve_exact(20)
        .map_err(|_| InspectError::Allocation)?;
    write!(&mut digits, "{position}").map_err(|_| InspectError::Allocation)?;
    let qualifier = Name::from_contents(space, limits()).map_err(|_| InspectError::Malformed)?;
    let name = Name::from_contents(&digits, limits()).map_err(|_| InspectError::Malformed)?;
    Ok(Expression::Field(FieldReference {
        qualifier: Some(qualifier),
        name,
    }))
}

fn lower(
    expression: &mut Expression,
    event: BeforeEvent,
    table: &TableSnapshot,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    check(budget, cancelled)?;
    match expression {
        Expression::Null
        | Expression::Boolean(_)
        | Expression::Integer(_)
        | Expression::Text(_) => {}
        Expression::Field(field) => {
            let (space, selected) = if let Some(qualifier) = &field.qualifier {
                if !qualifier.as_str().eq_ignore_ascii_case("Old") || event == BeforeEvent::Insert {
                    return Err(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Context,
                    ));
                }
                (
                    OLD,
                    position(field.name.as_str(), table, budget, cancelled)?,
                )
            } else if field.name.as_str().eq_ignore_ascii_case("IsInsert") {
                if event == BeforeEvent::Delete {
                    return Err(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Context,
                    ));
                }
                (STATE, 0)
            } else {
                if event == BeforeEvent::Delete {
                    return Err(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Context,
                    ));
                }
                (
                    CURRENT,
                    position(field.name.as_str(), table, budget, cancelled)?,
                )
            };
            *expression = binding(space, selected, budget)?;
        }
        Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
            lower(expression, event, table, budget, cancelled)?;
        }
        Expression::Binary { left, right, .. } => {
            lower(left, event, table, budget, cancelled)?;
            lower(right, event, table, budget, cancelled)?;
        }
        Expression::Call {
            qualifier,
            name,
            arguments,
        } => {
            if !qualifier.is_empty() {
                return Err(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::Expression,
                ));
            }
            if name.as_str().eq_ignore_ascii_case("Updated") {
                if event != BeforeEvent::Update {
                    return Err(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Context,
                    ));
                }
                let [FunctionArgument::Expression(Expression::Text(name))] = arguments.as_slice()
                else {
                    return Err(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Expression,
                    ));
                };
                let selected = position(name, table, budget, cancelled)?;
                *expression = binding(UPDATED, selected, budget)?;
            } else if name.as_str().eq_ignore_ascii_case("IsNull") {
                let [FunctionArgument::Expression(value)] = arguments.as_mut_slice() else {
                    return Err(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Expression,
                    ));
                };
                lower(value, event, table, budget, cancelled)?;
            } else {
                return Err(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::Expression,
                ));
            }
        }
        // No dynamic Eval, query/domain/external references, runtime/session,
        // clock, random, member access or unreviewed scalar calls are installed.
        _ => {
            return Err(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::Expression,
            ));
        }
    }
    Ok(())
}
