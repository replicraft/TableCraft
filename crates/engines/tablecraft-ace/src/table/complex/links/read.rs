use super::super::{ComplexTableSchema, inspect_complex_schema, supported_key};
use super::{membership::Parents, metadata::ComplexColumnLinks};
use crate::{
    InspectError, TableCursor, TableSnapshotLimits,
    header::check,
    table::{page, snapshot},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

/// Resolve one complex column and read its parent keys and raw flat children on
/// a stable/offline source. Assigned parent keys and child foreign keys must be
/// positive; child identities retain every signed Int32 value. Parent keys and
/// child identities are unique in their own scopes. Absent parent keys retain
/// NULL and empty membership.
/// Rejects orphan/ambiguous/NULL child references without partial publication.
/// No expressions, scripts, providers or attachment decompression execute.
pub fn read_complex_column_links<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ComplexColumnLinks, InspectError> {
    read_complex_column_links_with_limits(
        source,
        scope,
        table,
        column,
        TableSnapshotLimits::default(),
        budget,
        cancelled,
    )
}

/// Read complete parent/child links with explicit row and internal batch limits.
/// Both tables use the same ceiling and cumulative controls; no partial links are
/// returned. This does not acquire a live-file snapshot or relax key validation.
pub fn read_complex_column_links_with_limits<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    limits: TableSnapshotLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ComplexColumnLinks, InspectError> {
    let _span = tracing::debug_span!("native.complex.links").entered();
    check(budget, cancelled)?;
    let schema = inspect_complex_schema(source, scope, table, budget, cancelled)?;
    let field_position = schema
        .owner()
        .columns()
        .iter()
        .position(|field| field.name().eq_ignore_ascii_case(column))
        .ok_or(InspectError::UnknownColumn)?;
    read_resolved(
        source,
        scope,
        schema,
        field_position,
        limits,
        budget,
        cancelled,
    )
}

pub(in crate::table::complex) fn read_resolved<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    schema: ComplexTableSchema,
    field_position: usize,
    limits: TableSnapshotLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ComplexColumnLinks, InspectError> {
    let (position, parents) =
        read_parents(source, &schema, field_position, limits, budget, cancelled)?;
    let flat = schema.columns()[position].flat_table();
    let declaration = &schema.columns()[position];
    let attachment =
        match super::super::attachments::schema::qualify(declaration, flat, budget, cancelled) {
            Ok(_) => true,
            Err(InspectError::Unsupported) => false,
            Err(error) => return Err(error),
        };
    let cursor = if attachment {
        let observed = crate::inspect_table_schema(source, scope, flat.name(), budget, cancelled)?;
        if !observed.same_definition(flat) {
            return Err(InspectError::Malformed);
        }
        let (_, columns) = super::super::attachments::schema::projection(
            declaration,
            &observed,
            None,
            budget,
            cancelled,
        )?;
        TableCursor::from_qualified(source, observed, None, columns, budget, cancelled)?
    } else {
        TableCursor::open(source, scope, flat.name(), budget, cancelled)?
    };
    let children = cursor.into_snapshot(limits, budget, cancelled)?;
    let expected = if attachment {
        super::super::attachments::schema::projection(
            declaration,
            children.schema(),
            None,
            budget,
            cancelled,
        )?
        .1
    } else {
        flat.scalar_columns_with_budget(budget)?
    };
    if children.schema().object_id() != flat.object_id()
        || children.schema().definition_page() != flat.definition_page()
        || children.schema().declared_rows() != flat.declared_rows()
        || children.columns() != expected
    {
        return Err(InspectError::Malformed);
    }
    let number_position = |number| {
        children
            .schema()
            .columns()
            .iter()
            .position(|field| field.number() == number)
            .ok_or(InspectError::Malformed)
    };
    let foreign_key = number_position(schema.columns()[position].foreign_key_number())?;
    let identity = number_position(schema.columns()[position].identity_number())?;
    let membership = parents.link(children.rows(), foreign_key, identity, budget, cancelled)?;
    Ok(ComplexColumnLinks {
        schema,
        position,
        parents: membership.parents,
        children,
        order: membership.order,
    })
}

pub(in crate::table::complex) fn read_parents<R: crate::NativeReadSource>(
    source: &mut R,
    schema: &ComplexTableSchema,
    field_position: usize,
    limits: TableSnapshotLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(usize, Parents), InspectError> {
    check(budget, cancelled)?;
    let field = schema
        .owner()
        .columns()
        .get(field_position)
        .ok_or(InspectError::Malformed)?;
    if !supported_key(field) {
        return Err(InspectError::Unsupported);
    }
    let position = schema
        .columns()
        .iter()
        .position(|complex| complex.number() == field.number())
        .ok_or(InspectError::Malformed)?;
    let carrier = snapshot::projected_column(field, ScalarType::Int32, budget)?;
    let rows = page::read_records_batched(
        source,
        schema.owner(),
        &[carrier],
        Some(&[field_position]),
        limits,
        budget,
        cancelled,
    )?;
    Ok((position, Parents::read(rows, budget, cancelled)?))
}
