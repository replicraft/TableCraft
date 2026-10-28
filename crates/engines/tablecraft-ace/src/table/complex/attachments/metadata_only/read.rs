use super::super::schema;
use super::AttachmentMetadataColumn;
use crate::table::complex::links;
use crate::{
    InspectError, NativeReadSource, ReadProfile, TableCursor, TableSnapshotLimits, header::check,
    inspect_complex_schema,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Read complete attachment metadata and checked memberships without decoding
/// FileData values or visiting their long-value pages. Requires a stable/offline
/// source; omitted payloads are not qualified or made available as NULL.
pub fn read_attachment_metadata<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AttachmentMetadataColumn, InspectError> {
    read_attachment_metadata_with_limits(
        source,
        scope,
        table,
        column,
        TableSnapshotLimits::default(),
        budget,
        cancelled,
    )
}

/// Read a complete metadata projection under explicit parent/child limits.
/// All parent keys, child references/identities, selected metadata and live counts
/// are checked before publication. FileData declarations must qualify, but their
/// values, wrappers and external pages are deliberately not read or validated.
pub fn read_attachment_metadata_with_limits<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    limits: TableSnapshotLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AttachmentMetadataColumn, InspectError> {
    let _span = tracing::debug_span!("native.complex.attachments.metadata").entered();
    let complex = inspect_complex_schema(source, scope, table, budget, cancelled)?;
    if !matches!(
        complex.owner().header.read_profile()?,
        ReadProfile::Ace2007 | ReadProfile::Ace2016 | ReadProfile::Ace2019
    ) {
        return Err(InspectError::Unsupported);
    }
    let field_position = complex
        .owner()
        .columns()
        .iter()
        .position(|field| field.name().eq_ignore_ascii_case(column))
        .ok_or(InspectError::UnknownColumn)?;
    let number = complex.owner().columns()[field_position].number();
    let position = complex
        .columns()
        .iter()
        .position(|field| field.number() == number)
        .ok_or(InspectError::Malformed)?;
    let declaration = &complex.columns()[position];
    let flat = declaration.flat_table();
    let qualified = schema::qualify(declaration, flat, budget, cancelled)?;
    let references = [
        declaration.foreign_key_number(),
        declaration.identity_number(),
    ];
    let count = schema::FIELD_COUNT - 1 + references.len();
    budget.charge_allocation(count as u64, size_of::<&str>() as u64)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for role in 0..schema::FIELD_COUNT {
        check(budget, cancelled)?;
        if role != schema::DATA {
            names.push(flat.columns()[qualified.positions[role]].name());
        }
    }
    for reference in references {
        check(budget, cancelled)?;
        names.push(
            flat.columns()
                .iter()
                .find(|field| field.number() == reference)
                .ok_or(InspectError::Malformed)?
                .name(),
        );
    }
    let (_, expected) = schema::projection(declaration, flat, Some(&names), budget, cancelled)?;
    let (observed_position, parents) =
        links::read_parents(source, &complex, field_position, limits, budget, cancelled)?;
    if observed_position != position {
        return Err(InspectError::Malformed);
    }
    let observed = crate::inspect_table_schema(source, scope, flat.name(), budget, cancelled)?;
    if observed.object_id() != flat.object_id()
        || observed.definition_page() != flat.definition_page()
        || observed.layout != flat.layout
    {
        return Err(InspectError::Malformed);
    }
    let (positions, columns) =
        schema::projection(declaration, &observed, Some(&names), budget, cancelled)?;
    let children = TableCursor::from_qualified(
        source,
        observed,
        Some(positions),
        columns,
        budget,
        cancelled,
    )?
    .into_snapshot(limits, budget, cancelled)?;
    if children.schema().object_id() != flat.object_id()
        || children.schema().definition_page() != flat.definition_page()
        || children.schema().declared_rows() != flat.declared_rows()
        || children.columns() != expected
    {
        return Err(InspectError::Malformed);
    }

    let observed = schema::qualify(declaration, children.schema(), budget, cancelled)?;
    let projected_position = |number| {
        let native = children
            .schema()
            .columns()
            .iter()
            .find(|field| field.number() == number)
            .ok_or(InspectError::Malformed)?;
        children
            .columns()
            .iter()
            .position(|field| field.name().eq_ignore_ascii_case(native.name()))
            .ok_or(InspectError::Malformed)
    };
    let mut positions = [None; schema::FIELD_COUNT];
    for (role, position) in positions.iter_mut().enumerate() {
        check(budget, cancelled)?;
        if role != schema::DATA {
            *position = Some(projected_position(
                children.schema().columns()[observed.positions[role]].number(),
            )?);
        }
    }
    let foreign_key = projected_position(declaration.foreign_key_number())?;
    let identity = projected_position(declaration.identity_number())?;
    let membership = parents.link(children.rows(), foreign_key, identity, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(AttachmentMetadataColumn {
        schema: complex,
        position,
        membership,
        children,
        positions,
    })
}
