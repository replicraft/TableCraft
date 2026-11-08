//! Lower exact initial-field properties through shared typed input and native admission.
mod number;

use crate::{Error, display::Presentation, editing::EditError, scalar_input};
use tablecraft_ace::{CreateColumn, FirstTableDefinition};
use tablecraft_builders::table::{
    DraftDefaultMode, DraftFieldType, SnapshotField, TableDraftSnapshot,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_schema::{ScalarType, TypeDescriptor};

const PRIMARY_INDEX_NAME: &str = "PrimaryKey";

fn scalar(field: &SnapshotField) -> Result<ScalarType, Error> {
    Ok(match field.kind() {
        DraftFieldType::AutoNumberIncrement => ScalarType::Int32,
        DraftFieldType::Number => {
            number::scalar(field.number_size().ok_or(EditError::Unavailable)?)
        }
        DraftFieldType::ShortText => ScalarType::Text,
        DraftFieldType::LargeInteger => ScalarType::Int64,
        DraftFieldType::YesNo => ScalarType::Boolean,
        DraftFieldType::Currency => ScalarType::Currency,
        DraftFieldType::DateTime => ScalarType::DateTime,
    })
}

fn type_default(field: &SnapshotField) -> Result<Option<Value>, Error> {
    Ok(match field.kind() {
        DraftFieldType::Number => return number::default(field),
        DraftFieldType::LargeInteger => Some(Value::Int64(0)),
        DraftFieldType::YesNo => Some(Value::Boolean(false)),
        DraftFieldType::Currency => Some(Value::Currency(Currency::from_scaled(0))),
        DraftFieldType::AutoNumberIncrement
        | DraftFieldType::ShortText
        | DraftFieldType::DateTime => None,
    })
}

pub(super) fn capture(
    snapshot: &TableDraftSnapshot,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<FirstTableDefinition, Error> {
    let mut input = Presentation::new(cancelled);
    let generated = snapshot
        .fields()
        .iter()
        .any(|field| field.kind() == DraftFieldType::AutoNumberIncrement);
    input.allocation(snapshot.fields().len(), size_of::<Option<Value>>())?;
    let mut defaults = Vec::new();
    defaults.try_reserve_exact(snapshot.fields().len())?;
    for field in snapshot.fields() {
        input.work(1)?;
        defaults.push(match field.default_mode() {
            DraftDefaultMode::TypeDefault => type_default(field)?,
            DraftDefaultMode::Absent => None,
            DraftDefaultMode::Null => Some(Value::Null),
            DraftDefaultMode::Literal(text) => Some(scalar_input::parse_declared(
                match field.decimal_constraints() {
                    Some(constraints) => {
                        TypeDescriptor::decimal(constraints.precision(), constraints.scale())
                            .map_err(|_| EditError::InvalidInput)?
                    }
                    None => TypeDescriptor::new(scalar(field)?),
                },
                text,
                false,
                &mut input,
            )?),
        });
    }

    work.charge_allocation(
        snapshot.fields().len() as u64,
        size_of::<CreateColumn<'_>>() as u64,
    )?;
    let mut columns = Vec::new();
    columns.try_reserve_exact(snapshot.fields().len())?;
    for (position, (field, default)) in snapshot.fields().iter().zip(&defaults).enumerate() {
        input.work(1)?;
        let mut column = match field.kind() {
            DraftFieldType::AutoNumberIncrement if position == 0 && field.is_primary() => {
                if field.required().is_some()
                    || field.allow_zero_length().is_some()
                    || !matches!(field.default_mode(), DraftDefaultMode::TypeDefault)
                {
                    return Err(EditError::Unavailable.into());
                }
                CreateColumn::new(field.name(), ScalarType::Int32)
                    .with_primary_key(PRIMARY_INDEX_NAME)?
                    .with_incrementing_autonumber()?
            }
            DraftFieldType::ShortText => CreateColumn::short_text(
                field.name(),
                usize::from(field.short_text_units().ok_or(EditError::Unavailable)?),
            )?,
            DraftFieldType::AutoNumberIncrement => return Err(EditError::Unavailable.into()),
            DraftFieldType::Number if field.decimal_constraints().is_some() => {
                CreateColumn::decimal(
                    field.name(),
                    field.decimal_constraints().ok_or(EditError::Unavailable)?,
                )
            }
            _ => CreateColumn::new(field.name(), scalar(field)?),
        };
        if field.kind() != DraftFieldType::AutoNumberIncrement && field.is_primary() {
            column = column.with_primary_key(PRIMARY_INDEX_NAME)?;
        }
        if let Some(required) = field.required() {
            column = column.with_required(required);
        }
        if let Some(allow) = field.allow_zero_length() {
            column = column.with_allow_zero_length(allow)?;
        }
        if let Some(value) = default {
            column = column.with_default_literal(value)?;
        }
        if let Some(description) = field.description() {
            column = column.with_description(description)?;
        }
        columns.push(column);
    }
    Ok(if generated {
        FirstTableDefinition::capture(snapshot.table_name(), &columns, work, cancelled)?
    } else {
        FirstTableDefinition::capture_manual(snapshot.table_name(), &columns, work, cancelled)?
    })
}
