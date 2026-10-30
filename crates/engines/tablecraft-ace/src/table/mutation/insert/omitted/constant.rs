//! Bounded literal defaults and separately authorized Date()/Now() binding.
use crate::{EditError, InspectError, NativeColumn, WriteBlocker, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Value},
};
use tablecraft_expressions::functions::dates::clock::ClockFunction;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_sql::{
    ParseErrorKind, ParseLimits, parse_expression_strict,
    select::{Expression, UnaryOperator},
};

const MAX_DEFAULT_UNITS: usize = 255;
const MAX_DEFAULT_BYTES: usize = MAX_DEFAULT_UNITS * size_of::<char>();
const MAX_DEFAULT_TOKENS: usize = 64;
const MAX_DEFAULT_DEPTH: usize = 16;

pub(super) fn resolve(
    column: &NativeColumn,
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    let expression = parse(text, budget, cancelled)?;
    assign(column, expression, budget, cancelled)
}

pub(super) enum Bound {
    Literal(Value),
    Clock(ClockFunction),
}

pub(super) fn bind_clock(
    column: &NativeColumn,
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound, EditError> {
    let expression = parse(text, budget, cancelled)?;
    if let Expression::Call {
        qualifier,
        name,
        arguments,
    } = &expression
    {
        if !qualifier.is_empty() || !arguments.is_empty() {
            return Err(unsupported());
        }
        let function = match ClockFunction::resolve(name.as_str()) {
            Some(function @ (ClockFunction::Date | ClockFunction::Now)) => function,
            _ => return Err(unsupported()),
        };
        if column.scalar_type() != Some(ScalarType::DateTime)
            || !column.fixed()
            || column.storage_length() != size_of::<f64>()
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        return Ok(Bound::Clock(function));
    }
    assign(column, expression, budget, cancelled).map(Bound::Literal)
}

fn parse(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Expression, EditError> {
    check(budget, cancelled)?;
    if text.len() > MAX_DEFAULT_BYTES {
        return Err(unsupported());
    }
    budget
        .charge(Resource::Steps, text.len() as u64)
        .map_err(InspectError::from)?;
    if text.encode_utf16().count() > MAX_DEFAULT_UNITS {
        return Err(unsupported());
    }
    budget
        .charge_allocation(MAX_DEFAULT_TOKENS as u64, size_of::<Expression>() as u64)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(text.len() as u64, 2)
        .map_err(InspectError::from)?;
    let expression = parse_expression_strict(
        text,
        ParseLimits {
            max_bytes: MAX_DEFAULT_BYTES,
            max_tokens: MAX_DEFAULT_TOKENS,
            max_depth: MAX_DEFAULT_DEPTH,
        },
    )
    .map_err(|error| {
        if error.kind == ParseErrorKind::Limit {
            InspectError::StructureLimit.into()
        } else {
            unsupported()
        }
    })?;
    check(budget, cancelled)?;

    Ok(expression)
}

fn assign(
    column: &NativeColumn,
    expression: Expression,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    let kind = column
        .scalar_type()
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    match expression {
        Expression::Null => Ok(Value::Null),
        Expression::Boolean(value) if kind == ScalarType::Boolean => Ok(Value::Boolean(value)),
        Expression::Text(value) if kind == ScalarType::Text => Ok(Value::Text(value)),
        Expression::Boolean(_) | Expression::Text(_) => {
            Err(EditError::Unsupported(WriteBlocker::Column))
        }
        expression => {
            let (coefficient, scale) = number(&expression, false, budget, cancelled)?;
            assign_number(column, coefficient, scale)
        }
    }
}

fn number(
    expression: &Expression,
    negative: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(i128, u8), EditError> {
    check(budget, cancelled)?;
    match expression {
        Expression::Unary {
            operator: UnaryOperator::Plus,
            expression,
        } => number(expression, negative, budget, cancelled),
        Expression::Unary {
            operator: UnaryOperator::Minus,
            expression,
        } => number(expression, !negative, budget, cancelled),
        Expression::Integer(value) => {
            let value = i128::from(*value);
            Ok((if negative { -value } else { value }, 0))
        }
        Expression::Number(text) => {
            let mut coefficient = 0_i128;
            let mut scale = 0_u8;
            let mut decimal = false;
            let mut digits = 0;
            for byte in text.bytes() {
                check(budget, cancelled)?;
                match byte {
                    b'.' if !decimal => decimal = true,
                    b'0'..=b'9' => {
                        coefficient = coefficient
                            .checked_mul(10)
                            .and_then(|value| value.checked_add(i128::from(byte - b'0')))
                            .ok_or_else(unsupported)?;
                        digits += 1;
                        if decimal {
                            scale = scale.checked_add(1).ok_or_else(unsupported)?;
                            if scale > Decimal::MAX_SCALE {
                                return Err(unsupported());
                            }
                        }
                    }
                    _ => return Err(unsupported()),
                }
            }
            if digits == 0 {
                return Err(unsupported());
            }
            Ok((if negative { -coefficient } else { coefficient }, scale))
        }
        _ => Err(unsupported()),
    }
}

fn assign_number(column: &NativeColumn, coefficient: i128, scale: u8) -> Result<Value, EditError> {
    let refused = || EditError::Unsupported(WriteBlocker::Column);
    let exact = |target_scale| {
        Decimal::new(coefficient, scale)
            .and_then(|value| value.rescale_exact(target_scale))
            .map_err(|_| refused())
    };
    match column.scalar_type() {
        Some(ScalarType::Byte) => u8::try_from(exact(0)?.coefficient())
            .map(Value::Byte)
            .map_err(|_| refused()),
        Some(ScalarType::Int16) => i16::try_from(exact(0)?.coefficient())
            .map(Value::Int16)
            .map_err(|_| refused()),
        Some(ScalarType::Int32) => i32::try_from(exact(0)?.coefficient())
            .map(Value::Int32)
            .map_err(|_| refused()),
        Some(ScalarType::Int64) => i64::try_from(exact(0)?.coefficient())
            .map(Value::Int64)
            .map_err(|_| refused()),
        Some(ScalarType::Currency) => {
            let scaled =
                i64::try_from(exact(Currency::SCALE)?.coefficient()).map_err(|_| refused())?;
            Ok(Value::Currency(Currency::from_scaled(scaled)))
        }
        Some(ScalarType::Decimal) => {
            let (precision, target_scale) = column.decimal_parameters().ok_or_else(refused)?;
            if !(1..=DecimalConstraints::MAX_PRECISION).contains(&precision)
                || target_scale > precision
            {
                return Err(refused());
            }
            let value = exact(target_scale)?;
            if value.coefficient().unsigned_abs() >= 10_u128.pow(u32::from(precision)) {
                return Err(refused());
            }
            Ok(Value::Decimal(value))
        }
        _ => Err(refused()),
    }
}

fn unsupported() -> EditError {
    EditError::Unsupported(WriteBlocker::Properties)
}
