use super::super::{
    BeforeEvent, BeforeEventGuardError, BeforeGuardRejection, BeforeGuardTrace,
    BeforeGuardTraceKind, buffer,
    compile::{Compiled, Operation},
    evaluate::{self as scalar, MAX_TRACE, RowBindings},
    source::Frame,
};
use super::{BeforeChangeAssignment, BeforeChangeFinalRow};
use crate::{InspectError, header::check};
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(super) struct Execution {
    pub trace: Vec<BeforeGuardTrace>,
    pub assignments: Vec<BeforeChangeAssignment>,
    pub rejection: Option<BeforeGuardRejection>,
    pub rows: Option<Vec<BeforeChangeFinalRow>>,
}

fn assignment(value: Value) -> Result<Value, BeforeEventGuardError> {
    let value = match value {
        Value::Null => return Ok(Value::Null),
        Value::Byte(value) => i32::from(value),
        Value::Int16(value) => i32::from(value),
        Value::Int32(value) => value,
        Value::Int64(value) => {
            i32::try_from(value).map_err(|_| BeforeEventGuardError::ArgumentType)?
        }
        _ => return Err(BeforeEventGuardError::ArgumentType),
    };
    Ok(Value::Int32(value))
}

pub(super) fn run(
    compiled: &Compiled,
    frames: &[Frame],
    event: BeforeEvent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Execution, BeforeEventGuardError> {
    check(budget, cancelled)?;
    let maximum = compiled
        .instructions
        .len()
        .checked_add(2)
        .and_then(|width| width.checked_mul(frames.len()))
        .and_then(|count| count.checked_add(2))
        .filter(|count| *count <= MAX_TRACE)
        .ok_or(InspectError::StructureLimit)?;
    let mut trace = buffer::vector(maximum, budget, cancelled)?;
    let mut assignments = buffer::vector(maximum, budget, cancelled)?;
    let mut rows = buffer::vector(frames.len(), budget, cancelled)?;
    scalar::trace(
        &mut trace,
        BeforeGuardTraceKind::CompiledAll,
        None,
        None,
        budget,
        cancelled,
    )?;

    for frame in frames {
        check(budget, cancelled)?;
        let proposed = frame.proposed.as_ref().ok_or(InspectError::Malformed)?;
        let mut current = buffer::vector(proposed.len(), budget, cancelled)?;
        for value in proposed {
            check(budget, cancelled)?;
            // The source/frame owner already admits only Int32 and NULL.
            if !matches!(value, Value::Int32(_) | Value::Null) {
                return Err(InspectError::Malformed.into());
            }
            current.push(value.clone());
        }
        scalar::trace(
            &mut trace,
            BeforeGuardTraceKind::Begin,
            Some(frame.ordinal),
            None,
            budget,
            cancelled,
        )?;
        let mut pc = 0;
        while let Some(instruction) = compiled.instructions.get(pc) {
            check(budget, cancelled)?;
            let bindings = RowBindings {
                frame,
                current: Some(&current),
                updated: &[],
                is_insert: Value::Boolean(event == BeforeEvent::Insert),
            };
            match &instruction.operation {
                Operation::SetField { column, expression } => {
                    let value =
                        assignment(scalar::scalar(expression, &bindings, budget, cancelled)?)?;
                    let selected = current.get_mut(*column).ok_or(InspectError::Malformed)?;
                    *selected = value;
                    buffer::push(
                        &mut assignments,
                        BeforeChangeAssignment {
                            ordinal: frame.ordinal,
                            column: *column,
                            program: instruction.program,
                            node: instruction.node,
                            span: instruction.span,
                            trace_position: trace.len(),
                        },
                        MAX_TRACE,
                        budget,
                        cancelled,
                    )?;
                    pc += 1;
                }
                Operation::If { expression, end } => {
                    let Value::Boolean(value) =
                        scalar::scalar(expression, &bindings, budget, cancelled)?
                    else {
                        return Err(BeforeEventGuardError::ConditionType);
                    };
                    scalar::trace(
                        &mut trace,
                        BeforeGuardTraceKind::Condition(value),
                        Some(frame.ordinal),
                        Some(instruction),
                        budget,
                        cancelled,
                    )?;
                    pc += 1;
                    if !value {
                        while let Some(next) = compiled.instructions.get(pc) {
                            check(budget, cancelled)?;
                            if next.program != instruction.program
                                || next.definition != instruction.definition
                                || next.span.start() >= *end
                            {
                                break;
                            }
                            pc += 1;
                        }
                    }
                }
                Operation::Raise {
                    number,
                    description,
                } => {
                    let rejection =
                        scalar::raise(number, description.as_ref(), &bindings, budget, cancelled)?;
                    scalar::trace(
                        &mut trace,
                        BeforeGuardTraceKind::Raised(rejection.number()),
                        Some(frame.ordinal),
                        Some(instruction),
                        budget,
                        cancelled,
                    )?;
                    scalar::trace(
                        &mut trace,
                        BeforeGuardTraceKind::Rejected,
                        None,
                        None,
                        budget,
                        cancelled,
                    )?;
                    check(budget, cancelled)?;
                    return Ok(Execution {
                        trace,
                        assignments,
                        rejection: Some(rejection),
                        rows: None,
                    });
                }
            }
        }
        scalar::trace(
            &mut trace,
            BeforeGuardTraceKind::End,
            Some(frame.ordinal),
            None,
            budget,
            cancelled,
        )?;
        rows.push(BeforeChangeFinalRow {
            ordinal: frame.ordinal,
            values: current,
        });
    }
    scalar::trace(
        &mut trace,
        BeforeGuardTraceKind::Passed,
        None,
        None,
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok(Execution {
        trace,
        assignments,
        rejection: None,
        rows: Some(rows),
    })
}
