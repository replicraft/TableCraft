//! Positive whole-image reconstruction of the bounded external-property history.
use super::{Error, build};
use crate::{
    CreateColumn, FirstTableDefinition, NativeImage, TableCheck, create_ace16_database,
    header::check,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

pub(in crate::creation::add_column::created) struct Source {
    pub(in crate::creation::add_column::created) checked: TableCheck,
    pub(in crate::creation::add_column::created) definition: FirstTableDefinition,
    pub(in crate::creation::add_column::created) default_first: bool,
}

pub(in crate::creation::add_column::created) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, Error> {
    check(budget, cancelled)?;
    let (checked, definition) =
        super::super::super::source::inspect(image, table, budget, cancelled)?;
    let count = checked.schema().columns().len();
    let rows = super::super::super::super::property_history::count(
        image,
        checked.schema(),
        count,
        budget,
        cancelled,
    )
    .map_err(|error| match error {
        super::super::super::super::property_history::PropertyHistoryError::Inspect(error) => {
            Error::Inspect(error)
        }
        super::super::super::super::property_history::PropertyHistoryError::Placement(error) => {
            Error::Placement(error)
        }
        super::super::super::super::property_history::PropertyHistoryError::Source => Error::Source,
    })?;

    let columns = definition.borrowed_columns(budget, cancelled)?;
    super::super::source_properties(&columns, budget, cancelled)?;
    let initial = count - rows;
    for field in &columns[initial..] {
        check(budget, cancelled)?;
        require_number(field)?;
    }
    let default_first = super::super::source_properties(&columns[..initial], budget, cancelled)?;
    let seed = create_ace16_database(table, &columns[..initial], &[], budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let seed = NativeImage::capture(&mut Cursor::new(seed.as_bytes()), scope, budget, cancelled)?;
    let first = seed.prepare_created_add_default_zero_int32_column_candidate(
        table,
        columns[initial].name(),
        super::super::CreatedDefaultZeroColumnRequired::StoredFalse,
        budget,
        cancelled,
    )?;
    let mut reconstructed = NativeImage::capture(
        &mut Cursor::new(first.after_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    for position in initial + 1..count {
        check(budget, cancelled)?;
        let (stage, _) =
            super::super::super::source::inspect(&reconstructed, table, budget, cancelled)?;
        let after = build::append(
            &reconstructed,
            &stage,
            &columns[..=position],
            default_first,
            budget,
            cancelled,
        )?;
        reconstructed = NativeImage::capture(&mut Cursor::new(after), scope, budget, cancelled)?;
    }
    if !super::super::super::super::common::equal_image(
        image.as_bytes(),
        reconstructed.as_bytes(),
        budget,
        cancelled,
    )? {
        return Err(Error::Source);
    }
    drop(columns);
    Ok(Source {
        checked,
        definition,
        default_first,
    })
}

fn require_number(field: &CreateColumn<'_>) -> Result<(), Error> {
    if field.scalar_type() != ScalarType::Int32
        || field.required() != Some(false)
        || field.default_literal() != Some(&Value::Int32(0))
    {
        return Err(Error::Source);
    }
    Ok(())
}
