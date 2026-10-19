use super::super::read_complex_column_links;
use super::MultivalueColumn;
use crate::{
    InspectError, NativeColumn,
    format::{
        data_type,
        layout::{FIXED_FLAG, UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use std::io::{Read, Seek};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

// Current bounded collection policy, matching Microsoft's documented product
// ceiling. Raw complex links retain their separate whole-table row ceiling.
const MAX_VALUES_PER_PARENT: usize = 100;

/// Resolve and read one ACE 2007/16-family stored multivalue collection column on a
/// stable/offline source. Reject unknown templates, incompatible scalar metadata,
/// ambiguous/orphan references and oversized collections before publication.
/// No lookup query, active content, SQL coercion or attachment decoding executes.
pub fn read_multivalue_column<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueColumn, InspectError> {
    let _span = tracing::debug_span!("native.complex.multivalue").entered();
    let links = read_complex_column_links(source, scope, table, column, budget, cancelled)?;
    read_links(links, budget, cancelled)
}

pub(in crate::table::complex) fn read_links(
    links: crate::ComplexColumnLinks,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueColumn, InspectError> {
    let [template] = links.column().type_table().columns() else {
        return Err(InspectError::Unsupported);
    };
    let kind = supported(template).ok_or(InspectError::Unsupported)?;
    let (value_position, flat) = links
        .child_snapshot()
        .schema()
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
    for parent in links.parents() {
        check(budget, cancelled)?;
        if parent.child_count() > MAX_VALUES_PER_PARENT {
            return Err(InspectError::StructureLimit);
        }
    }
    check(budget, cancelled)?;
    Ok(MultivalueColumn {
        links,
        value_position,
    })
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
