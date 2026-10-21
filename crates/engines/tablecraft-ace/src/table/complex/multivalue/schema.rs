//! Qualified scalar collection metadata without reading parent or child records.
use super::super::{ComplexColumnSchema, inspect_complex_schema, supported_key};
use crate::{
    InspectError, NativeColumn,
    format::{
        data_type,
        layout::{FIXED_FLAG, UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::{Column, ScalarType};

/// Inspect qualified multivalue element declarations, named for parent fields.
/// Non-multivalue/unsupported templates remain absent; corrupt resolved mappings
/// fail. No parent/child records, lookup queries or active content are read/run.
pub fn inspect_multivalue_columns<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Column>, InspectError> {
    inspect_declarations(source, scope, table, budget, cancelled, |column, _| column)
}

/// Inspect qualified element declarations with exact flat text-order metadata.
/// Unsupported templates remain absent; corrupt resolved mappings fail.
/// Reads no parent/child records and never chooses a database/template fallback
/// or runs lookup content. Unknown/zero descriptors remain metadata.
pub fn inspect_multivalue_declarations<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<super::MultivalueDeclaration>, InspectError> {
    inspect_declarations(source, scope, table, budget, cancelled, |column, order| {
        super::MultivalueDeclaration { column, order }
    })
}

fn inspect_declarations<R: crate::NativeReadSource, T>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    mut declaration: impl FnMut(Column, Option<crate::NativeSortOrder>) -> T,
) -> Result<Vec<T>, InspectError> {
    let _span = tracing::debug_span!("native.complex.multivalue.schema").entered();
    let schema = inspect_complex_schema(source, scope, table, budget, cancelled)?;
    budget.charge_allocation(schema.columns().len() as u64, size_of::<T>() as u64)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    for complex in schema.columns() {
        check(budget, cancelled)?;
        let field = schema
            .owner()
            .columns()
            .iter()
            .find(|field| field.number() == complex.number())
            .ok_or(InspectError::Malformed)?;
        if !supported_key(field) {
            continue;
        }
        let (position, kind) = match qualified(complex, complex.flat_table()) {
            Ok(value) => value,
            Err(InspectError::Unsupported) => continue,
            Err(error) => return Err(error),
        };
        let flat = &complex.flat_table().columns()[position];
        budget.charge_allocation(field.name().len() as u64, 1)?;
        let mut name = String::new();
        name.try_reserve_exact(field.name().len())
            .map_err(|_| InspectError::Allocation)?;
        name.push_str(field.name());
        columns.push(declaration(
            Column::with_type(name, flat.column_type(kind)?, true)?,
            flat.text_sort_order(),
        ));
    }
    check(budget, cancelled)?;
    Ok(columns)
}
pub(super) fn qualified(
    column: &ComplexColumnSchema,
    flat_table: &crate::TableSchema,
) -> Result<(usize, ScalarType), InspectError> {
    let [template] = column.type_table().columns() else {
        return Err(InspectError::Unsupported);
    };
    let kind = supported(template).ok_or(InspectError::Unsupported)?;
    let (position, flat) = flat_table
        .columns()
        .iter()
        .enumerate()
        .find(|(_, field)| field.name().eq_ignore_ascii_case(template.name()))
        .ok_or(InspectError::Malformed)?;
    if supported(flat) != Some(kind)
        || template.decimal_parameters() != flat.decimal_parameters()
        || template.fixed() != flat.fixed()
    {
        return Err(InspectError::Unsupported);
    }
    Ok((position, kind))
}

fn supported(column: &NativeColumn) -> Option<ScalarType> {
    let flags = UPDATABLE_FLAG | if column.fixed() { FIXED_FLAG } else { 0 };
    if column.flags() != flags
        || column.native_type() == data_type::LONG_TEXT
        || (column.extended_flags() != 0
            && !(column.native_type() == data_type::SHORT_TEXT
                && column.extended_flags() == UNICODE_COMPRESSION_FLAG))
    {
        return None;
    }
    let kind = if super::super::value_schema::conceptual_decimal(column) {
        ScalarType::Decimal
    } else {
        column.scalar_type()?
    };
    matches!(
        kind,
        ScalarType::Byte
            | ScalarType::Int16
            | ScalarType::Int32
            | ScalarType::Int64
            | ScalarType::Single
            | ScalarType::Double
            | ScalarType::Guid
            | ScalarType::Decimal
            | ScalarType::Text
    )
    .then_some(kind)
}
