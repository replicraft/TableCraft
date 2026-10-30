//! Audit literal projection is independent of producer assignment/rescale code.
use crate::{
    EditError, InspectError, NativeColumn, WriteBlocker, header::check,
    table::mutation::insert::default_clock,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Value},
};
use tablecraft_expressions::functions::dates::clock::{ClockFunction, ClockReading};
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_sql::{
    ParseErrorKind, ParseLimits, parse_expression_strict,
    select::{Expression, UnaryOperator},
};

const PROPERTY_UNITS: usize = 255;
const PROPERTY_BYTES: usize = PROPERTY_UNITS * 4;
const TOKEN_LIMIT: usize = 64;
const DEPTH_LIMIT: usize = 16;

pub(super) fn derive(
    column: &NativeColumn,
    text: &str,
    reading: Option<ClockReading>,
    used_clock: &mut bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    check(budget, cancelled)?;
    if text.len() > PROPERTY_BYTES {
        return Err(property());
    }
    budget
        .charge(Resource::Steps, text.len() as u64)
        .map_err(InspectError::from)?;
    if text.encode_utf16().count() > PROPERTY_UNITS {
        return Err(property());
    }
    budget
        .charge_allocation(TOKEN_LIMIT as u64, size_of::<Expression>() as u64)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(text.len() as u64, 2)
        .map_err(InspectError::from)?;
    let mut expression = parse_expression_strict(
        text,
        ParseLimits {
            max_bytes: PROPERTY_BYTES,
            max_tokens: TOKEN_LIMIT,
            max_depth: DEPTH_LIMIT,
        },
    )
    .map_err(|error| {
        if error.kind == ParseErrorKind::Limit {
            EditError::from(InspectError::StructureLimit)
        } else {
            property()
        }
    })?;
    let kind = column.scalar_type().ok_or_else(carrier)?;
    // Independently bind the original source call, never a producer default/cache.
    if let Expression::Call {
        qualifier,
        name,
        arguments,
    } = &expression
    {
        if !qualifier.is_empty() || !arguments.is_empty() {
            return Err(property());
        }
        let function = if name.as_str().eq_ignore_ascii_case("Date") {
            ClockFunction::Date
        } else if name.as_str().eq_ignore_ascii_case("Now") {
            ClockFunction::Now
        } else {
            return Err(property());
        };
        let reading = reading.ok_or_else(property)?;
        if kind != ScalarType::DateTime
            || !column.fixed()
            || column.storage_length() != size_of::<i64>()
        {
            return Err(carrier());
        }
        *used_clock = true;
        return default_clock::project(function, reading, budget, cancelled);
    }
    let mut sign = 1_i128;
    let mut numeric_unary = false;
    loop {
        check(budget, cancelled)?;
        match expression {
            Expression::Unary {
                operator: UnaryOperator::Plus,
                expression: child,
            } => {
                numeric_unary = true;
                expression = *child;
            }
            Expression::Unary {
                operator: UnaryOperator::Minus,
                expression: child,
            } => {
                numeric_unary = true;
                sign = -sign;
                expression = *child;
            }
            _ => break,
        }
    }
    let (coefficient, scale) = match expression {
        Expression::Null if !numeric_unary => return Ok(Value::Null),
        Expression::Boolean(value) if !numeric_unary => {
            return if kind == ScalarType::Boolean {
                Ok(Value::Boolean(value))
            } else {
                Err(carrier())
            };
        }
        Expression::Text(value) if !numeric_unary => {
            return if kind == ScalarType::Text {
                Ok(Value::Text(value))
            } else {
                Err(carrier())
            };
        }
        Expression::Integer(value) => (i128::from(value) * sign, 0_u8),
        Expression::Number(text) => {
            let mut magnitude = 0_i128;
            let mut fraction = None;
            let mut digits = 0;
            for byte in text.bytes() {
                check(budget, cancelled)?;
                if byte == b'.' && fraction.is_none() {
                    fraction = Some(0_u8);
                } else if byte.is_ascii_digit() {
                    magnitude = magnitude
                        .checked_mul(10)
                        .and_then(|value| value.checked_add(i128::from(byte - b'0')))
                        .ok_or_else(property)?;
                    digits += 1;
                    if let Some(scale) = &mut fraction {
                        *scale = scale.checked_add(1).ok_or_else(property)?;
                        if *scale > Decimal::MAX_SCALE {
                            return Err(property());
                        }
                    }
                } else {
                    return Err(property());
                }
            }
            if digits == 0 {
                return Err(property());
            }
            (magnitude * sign, fraction.unwrap_or(0))
        }
        _ => return Err(property()),
    };
    let scaled = |wanted: u8| -> Result<i128, EditError> {
        if wanted >= scale {
            coefficient
                .checked_mul(10_i128.pow(u32::from(wanted - scale)))
                .ok_or_else(carrier)
        } else {
            let divisor = 10_i128.pow(u32::from(scale - wanted));
            if coefficient % divisor != 0 {
                return Err(carrier());
            }
            Ok(coefficient / divisor)
        }
    };
    match kind {
        ScalarType::Byte => u8::try_from(scaled(0)?)
            .map(Value::Byte)
            .map_err(|_| carrier()),
        ScalarType::Int16 => i16::try_from(scaled(0)?)
            .map(Value::Int16)
            .map_err(|_| carrier()),
        ScalarType::Int32 => i32::try_from(scaled(0)?)
            .map(Value::Int32)
            .map_err(|_| carrier()),
        ScalarType::Int64 => i64::try_from(scaled(0)?)
            .map(Value::Int64)
            .map_err(|_| carrier()),
        ScalarType::Currency => {
            let value = i64::try_from(scaled(Currency::SCALE)?).map_err(|_| carrier())?;
            Ok(Value::Currency(Currency::from_scaled(value)))
        }
        ScalarType::Decimal => {
            let (precision, scale) = column.decimal_parameters().ok_or_else(carrier)?;
            if precision == 0 || precision > DecimalConstraints::MAX_PRECISION || scale > precision
            {
                return Err(carrier());
            }
            let coefficient = scaled(scale)?;
            if coefficient.unsigned_abs() >= 10_u128.pow(u32::from(precision)) {
                return Err(carrier());
            }
            Decimal::new(coefficient, scale)
                .map(Value::Decimal)
                .map_err(|_| carrier())
        }
        _ => Err(carrier()),
    }
}

fn property() -> EditError {
    EditError::Unsupported(WriteBlocker::Properties)
}

fn carrier() -> EditError {
    EditError::Unsupported(WriteBlocker::Column)
}
