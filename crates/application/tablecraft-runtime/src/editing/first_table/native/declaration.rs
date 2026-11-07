//! Lower exact initial-field properties through shared typed input and native admission.
use crate::{Error, display::Presentation, editing::EditError, scalar_input};
use tablecraft_ace::{CreateColumn, FirstTableDefinition};
use tablecraft_builders::table::{DraftDefaultMode, DraftFieldType, TableDraftSnapshot};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_schema::ScalarType;

const PRIMARY_INDEX_NAME: &str = "PrimaryKey";

fn scalar(kind: DraftFieldType) -> ScalarType {
    match kind {
        DraftFieldType::AutoNumberIncrement | DraftFieldType::LongInteger => ScalarType::Int32,
        DraftFieldType::ShortText => ScalarType::Text,
        DraftFieldType::LargeInteger => ScalarType::Int64,
        DraftFieldType::YesNo => ScalarType::Boolean,
        DraftFieldType::Currency => ScalarType::Currency,
        DraftFieldType::DateTime => ScalarType::DateTime,
    }
}

fn type_default(kind: DraftFieldType) -> Option<Value> {
    match kind {
        DraftFieldType::LongInteger => Some(Value::Int32(0)),
        DraftFieldType::LargeInteger => Some(Value::Int64(0)),
        DraftFieldType::YesNo => Some(Value::Boolean(false)),
        DraftFieldType::Currency => Some(Value::Currency(Currency::from_scaled(0))),
        DraftFieldType::AutoNumberIncrement
        | DraftFieldType::ShortText
        | DraftFieldType::DateTime => None,
    }
}

pub(super) fn capture(
    snapshot: &TableDraftSnapshot,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<FirstTableDefinition, Error> {
    let mut input = Presentation::new(cancelled);
    input.allocation(snapshot.fields().len(), size_of::<Option<Value>>())?;
    let mut defaults = Vec::new();
    defaults.try_reserve_exact(snapshot.fields().len())?;
    for field in snapshot.fields() {
        input.work(1)?;
        defaults.push(match field.default_mode() {
            DraftDefaultMode::TypeDefault => type_default(field.kind()),
            DraftDefaultMode::Absent => None,
            DraftDefaultMode::Null => Some(Value::Null),
            DraftDefaultMode::Literal(text) => Some(scalar_input::parse(
                scalar(field.kind()),
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
            DraftFieldType::ShortText if !field.is_primary() => CreateColumn::short_text(
                field.name(),
                usize::from(field.short_text_units().ok_or(EditError::Unavailable)?),
            )?,
            DraftFieldType::AutoNumberIncrement => return Err(EditError::Unavailable.into()),
            kind if !field.is_primary() => CreateColumn::new(field.name(), scalar(kind)),
            _ => return Err(EditError::Unavailable.into()),
        };
        if let Some(required) = field.required() {
            column = column.with_required(required);
        }
        if let Some(allow) = field.allow_zero_length() {
            column = column.with_allow_zero_length(allow)?;
        }
        if let Some(value) = default {
            column = column.with_default_literal(value)?;
        }
        columns.push(column);
    }
    Ok(FirstTableDefinition::capture(
        snapshot.table_name(),
        &columns,
        work,
        cancelled,
    )?)
}
