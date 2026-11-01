//! Original metadata chooses the new profile before any mutation is attempted.
use super::super::{event_error, mixed};
use crate::{
    EditError, FileFormat, IndexInspection, InspectError, NativeColumn, NativeImage, TableSchema,
    format::{
        data_type::{BOOLEAN, COMPLEX, CURRENCY, DATE_TIME, INT32, INT64, LONG_TEXT, OLE},
        layout::{COMPLEX_FK_EXT_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    inspect_table_indexes, inspect_table_schema,
    table::mutation::{guards::native_events, preserved_source::Source},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) fn selected(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    if !matches!(
        image.catalog().header().format(),
        FileFormat::Ace2016 | FileFormat::Ace2019
    ) {
        return Ok(false);
    }
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if !ordinary_schema(&schema, budget, cancelled)? {
        // Actual scalar decoding alone does not qualify complex references or
        // Memo/OLE storage for this new writer. Preserve the old Int32 service.
        return Ok(false);
    }
    let mut has_wide = false;
    for field in schema.columns() {
        check(budget, cancelled)?;
        if field.native_type() != INT32 && ordinary_fixed(field) {
            has_wide = true;
        }
    }
    if !has_wide {
        // In particular, old all-I32 composite/tree contexts do not enter
        // the narrower preserved Source at all.
        return Ok(false);
    }

    let indexes = inspect_table_indexes(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if !legacy_indexes(&schema, &indexes, budget, cancelled)? {
        // Preserve the existing checked-index service without attempting a
        // legacy single-key capture and catching its failure.
        return Ok(false);
    }
    let macro_targets =
        native_events::potential_targets(image, &schema, budget, cancelled).map_err(event_error)?;
    for position in macro_targets {
        check(budget, cancelled)?;
        let field = schema
            .columns()
            .get(position)
            .ok_or(InspectError::Malformed)?;
        if field.native_type() != INT32
            || !ordinary_fixed(field)
            || indexed(&indexes, field.number(), budget, cancelled)?
        {
            return Ok(false);
        }
    }

    let source = mixed::capture(image, table, budget, cancelled)?;
    if source.table.columns().len() != schema.columns().len() {
        return Ok(false);
    }
    for position in 0..schema.columns().len() {
        check(budget, cancelled)?;
        if eligible_wide(&source, position, budget, cancelled)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// One shared new-route scope check, before Source capture or event binding.
/// Readable FK markers and LVAL scalar carriers do not establish the required
/// complex/long-value qualification. This does not narrow any existing reader
/// or the separate preserved rich Int32 service.
pub(super) fn ordinary_schema(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    for field in schema.columns() {
        check(budget, cancelled)?;
        if field.scalar_type().is_none() {
            return Ok(false);
        }
        if matches!(field.native_type(), COMPLEX | LONG_TEXT | OLE) {
            return Ok(false);
        }
        if field.complex_type_id().is_some() || field.extended_flags() & COMPLEX_FK_EXT_FLAG != 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn eligible_wide(
    source: &Source,
    position: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    if source.table.schema().columns()[position].native_type() == INT32 {
        return Ok(false);
    }
    eligible(source, position, budget, cancelled)
}

fn eligible(
    source: &Source,
    position: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let field = &source.table.schema().columns()[position];
    if !ordinary_fixed(field) {
        return Ok(false);
    }
    for dependency in &source.dependencies {
        check(budget, cancelled)?;
        if *dependency == position {
            return Ok(false);
        }
    }
    Ok(!indexed(
        &source.indexes,
        field.number(),
        budget,
        cancelled,
    )?)
}

fn ordinary_fixed(field: &NativeColumn) -> bool {
    let width = match (field.native_type(), field.scalar_type()) {
        (BOOLEAN, Some(ScalarType::Boolean)) => 0,
        (INT32, Some(ScalarType::Int32)) => size_of::<i32>(),
        (INT64, Some(ScalarType::Int64)) => size_of::<i64>(),
        (CURRENCY, Some(ScalarType::Currency)) => size_of::<i64>(),
        (DATE_TIME, Some(ScalarType::DateTime)) => size_of::<f64>(),
        _ => return false,
    };
    if !field.fixed()
        || field.storage_length() != width
        || field.extended_flags() != 0
        || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
    {
        return false;
    }
    true
}

fn indexed(
    indexes: &IndexInspection,
    number: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    for index in indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == number {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// This is route classification, not index validity or absence authority. The
/// new unindexed fixed route uses preserved Source's single-I32-key observer;
/// every other shape stays with its existing checked service from metadata.
fn legacy_indexes(
    schema: &TableSchema,
    indexes: &IndexInspection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    for index in indexes.indexes() {
        check(budget, cancelled)?;
        let [key] = index.columns() else {
            return Ok(false);
        };
        let field = schema
            .columns()
            .get(key.number())
            .ok_or(InspectError::Malformed)?;
        if field.native_type() != INT32
            || field.scalar_type() != Some(ScalarType::Int32)
            || index.native_type() > 1
        {
            return Ok(false);
        }
        let Some(constraints) = index.constraints() else {
            return Ok(false);
        };
        if constraints.ignore_nulls() {
            return Ok(false);
        }
    }
    Ok(true)
}
