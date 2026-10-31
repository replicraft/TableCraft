use super::{
    BeforeEvent, BeforeEventGuardError, BeforeGuardRejection, BeforeGuardTrace,
    BeforeGuardTraceKind, buffer,
    compile::{Compiled, Description, Instruction, Operation},
    error, expression,
    source::Frame,
};
use crate::{InspectError, header::check};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    evaluation::{Bindings, EvalLimits, Evaluator},
};
use tablecraft_sql::select::{Expression, FieldReference};

pub(super) const MAX_TRACE: usize = 65_536;

pub(super) struct RowBindings<'a> {
    pub frame: &'a Frame,
    pub updated: &'a [Value],
    pub current: Option<&'a [Value]>,
    pub is_insert: Value,
}

impl Bindings for RowBindings<'_> {
    fn resolve(&self, reference: &FieldReference) -> Result<&Value, EvalError> {
        let position = reference
            .name
            .as_str()
            .parse::<usize>()
            .map_err(|_| EvalError::UnboundField)?;
        match reference.qualifier.as_ref().map(|name| name.as_str()) {
            Some(expression::CURRENT) => self
                .current
                .or(self.frame.proposed.as_deref())
                .and_then(|row| row.get(position)),
            Some(expression::OLD) => self
                .frame
                .original
                .as_ref()
                .and_then(|row| row.get(position)),
            Some(expression::UPDATED) => self.updated.get(position),
            Some(expression::STATE) if position == 0 => Some(&self.is_insert),
            _ => None,
        }
        .ok_or(EvalError::UnboundField)
    }
}

pub(super) fn scalar(
    expression: &Expression,
    bindings: &RowBindings<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, BeforeEventGuardError> {
    Evaluator::new(bindings, budget, EvalLimits::default())
        .with_cancellation(cancelled)
        .evaluate(expression)
        .map_err(error::evaluation)
}

pub(super) fn trace(
    output: &mut Vec<BeforeGuardTrace>,
    kind: BeforeGuardTraceKind,
    row: Option<usize>,
    instruction: Option<&Instruction>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    buffer::push(
        output,
        BeforeGuardTrace {
            kind,
            row,
            program: instruction.map(|value| value.program),
            node: instruction.map(|value| value.node),
            span: instruction.map(|value| value.span),
        },
        MAX_TRACE,
        budget,
        cancelled,
    )?;
    Ok(())
}

pub(super) fn run(
    compiled: &Compiled,
    frames: &[Frame],
    event: BeforeEvent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<BeforeGuardTrace>, Option<BeforeGuardRejection>), BeforeEventGuardError> {
    check(budget, cancelled)?;
    let maximum = compiled
        .instructions
        .len()
        .checked_add(2)
        .and_then(|per_row| per_row.checked_mul(frames.len()))
        .and_then(|rows| rows.checked_add(2))
        .ok_or(InspectError::StructureLimit)?;
    if maximum > MAX_TRACE {
        return Err(InspectError::StructureLimit.into());
    }
    let mut output = buffer::vector(maximum, budget, cancelled)?;
    trace(
        &mut output,
        BeforeGuardTraceKind::CompiledAll,
        None,
        None,
        budget,
        cancelled,
    )?;

    for frame in frames {
        check(budget, cancelled)?;
        let width = frame
            .proposed
            .as_ref()
            .or(frame.original.as_ref())
            .map_or(0, Vec::len);
        let mut updated = buffer::vector(width, budget, cancelled)?;
        for position in 0..width {
            check(budget, cancelled)?;
            let changed = match (&frame.original, &frame.proposed) {
                (Some(old), Some(new)) => old.get(position) != new.get(position),
                _ => false,
            };
            updated.push(Value::Boolean(changed));
        }
        let bindings = RowBindings {
            frame,
            updated: &updated,
            current: None,
            is_insert: Value::Boolean(event == BeforeEvent::Insert),
        };
        trace(
            &mut output,
            BeforeGuardTraceKind::Begin,
            Some(frame.ordinal),
            None,
            budget,
            cancelled,
        )?;
        let mut pc = 0;
        while let Some(instruction) = compiled.instructions.get(pc) {
            check(budget, cancelled)?;
            match &instruction.operation {
                Operation::SetField { .. } => {
                    return Err(BeforeEventGuardError::Unsupported(
                        super::BeforeGuardUnsupported::Context,
                    ));
                }
                Operation::If { expression, end } => {
                    let Value::Boolean(value) = scalar(expression, &bindings, budget, cancelled)?
                    else {
                        return Err(BeforeEventGuardError::ConditionType);
                    };
                    trace(
                        &mut output,
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
                        raise(number, description.as_ref(), &bindings, budget, cancelled)?;
                    let number = rejection.number;
                    trace(
                        &mut output,
                        BeforeGuardTraceKind::Raised(number),
                        Some(frame.ordinal),
                        Some(instruction),
                        budget,
                        cancelled,
                    )?;
                    trace(
                        &mut output,
                        BeforeGuardTraceKind::Rejected,
                        None,
                        None,
                        budget,
                        cancelled,
                    )?;
                    check(budget, cancelled)?;
                    return Ok((output, Some(rejection)));
                }
            }
        }
        trace(
            &mut output,
            BeforeGuardTraceKind::End,
            Some(frame.ordinal),
            None,
            budget,
            cancelled,
        )?;
    }
    trace(
        &mut output,
        BeforeGuardTraceKind::Passed,
        None,
        None,
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok((output, None))
}

pub(super) fn raise(
    number: &Expression,
    description: Option<&Description>,
    bindings: &RowBindings<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BeforeGuardRejection, BeforeEventGuardError> {
    let number = match scalar(number, bindings, budget, cancelled)? {
        Value::Byte(value) => i32::from(value),
        Value::Int16(value) => i32::from(value),
        Value::Int32(value) => value,
        Value::Int64(value) => {
            i32::try_from(value).map_err(|_| BeforeEventGuardError::ArgumentType)?
        }
        _ => return Err(BeforeEventGuardError::ArgumentType),
    };
    let description = match description {
        None => None,
        Some(Description::Expression(expression)) => {
            let Value::Text(value) = scalar(expression, bindings, budget, cancelled)? else {
                return Err(BeforeEventGuardError::ArgumentType);
            };
            Some(value)
        }
        Some(Description::Literal(value)) => {
            let mut bytes = buffer::vector(value.len(), budget, cancelled)?;
            bytes.extend_from_slice(value.as_bytes());
            Some(String::from_utf8(bytes).map_err(|_| InspectError::Malformed)?)
        }
    };
    Ok(BeforeGuardRejection {
        number,
        description,
    })
}
