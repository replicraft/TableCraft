//! Original typed selection values and eager SQL comparison semantics.
use super::{Criterion, FilterComparison, Predicate};
use crate::{Error, controls::check};
use tablecraft_ace::{NativeColumn, TableSchema};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::comparison::StringInput;
use tablecraft_expressions::{
    comparison::{Collation, Comparison, StringSearch, compare},
    logic::Truth,
    predicates::{PreparedLikeSearch, between},
};
use tablecraft_schema::{ScalarType, TypeDescriptor};

pub(super) fn require_field(field: &NativeColumn) -> Result<(), Error> {
    // A complex field's internal Int32 foreign key is not the visible value.
    // The same admission applies when a definition binds to a reopened schema.
    if field.complex_type_id().is_some()
        || field.scalar_type().is_none()
        || field.scalar_type() == Some(ScalarType::Binary)
    {
        return Err(super::FilterError::UnsupportedField.into());
    }
    Ok(())
}

pub(super) fn copy_text(
    text: &str,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, Error> {
    check(cancelled)?;
    work.charge(Resource::Steps, 1 + text.len() as u64)?;
    work.charge_allocation(text.len() as u64, 1)?;
    let mut owned = String::new();
    owned.try_reserve_exact(text.len())?;
    owned.push_str(text);
    Ok(owned)
}

pub(super) fn copy_value(
    value: &Value,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, Error> {
    check(cancelled)?;
    work.charge_allocation(1, size_of::<Value>() as u64)?;
    match value {
        Value::Text(text) => Ok(Value::Text(copy_text(text, work, cancelled)?)),
        Value::Binary(_) => Err(super::FilterError::UnsupportedField.into()),
        _ => Ok(value.clone()),
    }
}

pub(super) fn selected(
    column: &str,
    value: &Value,
    comparison: FilterComparison,
    declaration: TypeDescriptor,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Criterion, Error> {
    let predicate = match (comparison, value) {
        (FilterComparison::Equal, Value::Null) => Predicate::IsNull,
        (FilterComparison::NotEqual, Value::Null) => Predicate::IsNotNull,
        (FilterComparison::Equal, value) => Predicate::Equal(copy_value(value, work, cancelled)?),
        (FilterComparison::NotEqual, value) => {
            Predicate::NotEqual(copy_value(value, work, cancelled)?)
        }
    };
    Ok(Criterion {
        column: copy_text(column, work, cancelled)?,
        declaration,
        predicate,
    })
}

pub(super) fn copy(
    criterion: &Criterion,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Criterion, Error> {
    let predicate = match &criterion.predicate {
        Predicate::IsNull => Predicate::IsNull,
        Predicate::IsNotNull => Predicate::IsNotNull,
        Predicate::Equal(value) => Predicate::Equal(copy_value(value, work, cancelled)?),
        Predicate::NotEqual(value) => Predicate::NotEqual(copy_value(value, work, cancelled)?),
        Predicate::Ordered {
            comparison,
            expected,
        } => Predicate::Ordered {
            comparison: *comparison,
            expected: copy_value(expected, work, cancelled)?,
        },
        Predicate::Between {
            first,
            second,
            negated,
        } => Predicate::Between {
            first: copy_value(first, work, cancelled)?,
            second: copy_value(second, work, cancelled)?,
            negated: *negated,
        },
        Predicate::Pattern { operator, input } => Predicate::Pattern {
            operator: *operator,
            input: copy_text(input, work, cancelled)?,
        },
    };
    Ok(Criterion {
        column: copy_text(&criterion.column, work, cancelled)?,
        declaration: criterion.declaration,
        predicate,
    })
}

pub(super) fn evaluate(
    criterion: &Criterion,
    value: &Value,
    collation: &dyn Collation,
    prepared: Option<&PreparedLikeSearch>,
    work: &mut ResourceBudget,
    pattern_work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Truth, Error> {
    check(cancelled)?;
    work.charge(Resource::Steps, 1)?;
    Ok(match &criterion.predicate {
        Predicate::IsNull => {
            if matches!(value, Value::Null) {
                Truth::True
            } else {
                Truth::False
            }
        }
        Predicate::IsNotNull => {
            if matches!(value, Value::Null) {
                Truth::False
            } else {
                Truth::True
            }
        }
        Predicate::Equal(expected) => compare(Comparison::Equal, value, expected, Some(collation))?,
        Predicate::NotEqual(expected) => {
            compare(Comparison::NotEqual, value, expected, Some(collation))?
        }
        Predicate::Ordered {
            comparison,
            expected,
        } => compare(*comparison, value, expected, Some(collation))?,
        Predicate::Between {
            first,
            second,
            negated,
        } => between(value, first, second, *negated, |comparison, left, right| {
            check(cancelled)?;
            work.charge(Resource::Steps, 1)?;
            compare(comparison, left, right, Some(collation)).map_err(Error::from)
        })?,
        Predicate::Pattern { operator, .. } => {
            if matches!(value, Value::Null) {
                return Ok(Truth::Unknown);
            }
            let Value::Text(text) = value else {
                return Err(super::FilterError::UnsupportedField.into());
            };
            let matcher = prepared.ok_or(super::FilterError::UnknownPatternMode)?;
            let matched = matcher
                .find(
                    StringInput::Text(text),
                    StringSearch::ForwardFrom(0),
                    collation,
                    pattern_work,
                    Some(cancelled),
                )?
                .is_some();
            if matched != operator.negated_pattern() {
                Truth::True
            } else {
                Truth::False
            }
        }
    })
}

pub(super) fn needs_text_policy(criterion: &Criterion, value: &Value) -> bool {
    matches!(
        (&criterion.predicate, value),
        (
            Predicate::Equal(Value::Text(_))
                | Predicate::NotEqual(Value::Text(_))
                | Predicate::Pattern { .. },
            Value::Text(_)
        )
    )
}

pub(super) fn field_position(
    schema: &TableSchema,
    name: &str,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, Error> {
    for (position, field) in schema.columns().iter().enumerate() {
        check(cancelled)?;
        work.charge(
            Resource::Steps,
            1 + field.name().len() as u64 + name.len() as u64,
        )?;
        if field.name() == name {
            return Ok(position);
        }
    }
    Err(super::FilterError::InvalidSelection.into())
}
