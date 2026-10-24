//! Metadata-only attachment member declarations. FileData stays opaque.
use super::schema;
use crate::{InspectError, header::check, inspect_complex_schema};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::{CollectionMember, Column};

/// Inspect qualified stored attachment members without loading parent/child rows.
/// FileData is declared as stored Binary; its wrapper is not decoded or interpreted.
/// Unsupported templates remain absent; corrupt resolved mappings fail.
pub fn inspect_attachment_members<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CollectionMember>, InspectError> {
    inspect_declarations(source, scope, table, budget, cancelled, |member, _| member)
}

/// Inspect qualified attachment members with exact flat text-order metadata.
/// Unsupported templates remain absent; corrupt resolved mappings fail.
/// FileData retains its flat Binary declaration. No parent/child records, payloads, paths, lookup content
/// or active objects are read/executed; unknown descriptors remain explicit.
pub fn inspect_attachment_declarations<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<super::AttachmentMemberDeclaration>, InspectError> {
    inspect_declarations(source, scope, table, budget, cancelled, |member, order| {
        super::AttachmentMemberDeclaration { member, order }
    })
}

fn inspect_declarations<R: crate::NativeReadSource, T>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    mut declaration: impl FnMut(CollectionMember, Option<crate::NativeSortOrder>) -> T,
) -> Result<Vec<T>, InspectError> {
    let _span = tracing::debug_span!("native.complex.attachments.schema").entered();
    let complex = inspect_complex_schema(source, scope, table, budget, cancelled)?;
    if !matches!(
        complex.owner().header.read_profile()?,
        crate::ReadProfile::Ace2007 | crate::ReadProfile::Ace2016 | crate::ReadProfile::Ace2019
    ) {
        return Err(InspectError::Unsupported);
    }
    let mut members = Vec::new();
    for column in complex.columns() {
        check(budget, cancelled)?;
        let owner = complex
            .owner()
            .columns()
            .iter()
            .find(|field| field.number() == column.number())
            .ok_or(InspectError::Malformed)?;
        if !super::super::supported_key(owner) {
            continue;
        }
        let positions = match schema::qualify(column, column.flat_table(), budget, cancelled) {
            Ok(value) => value,
            Err(InspectError::Unsupported) => continue,
            Err(error) => return Err(error),
        };
        for (index, (name, _)) in schema::FIELDS.iter().enumerate() {
            check(budget, cancelled)?;
            let flat = &column.flat_table().columns()[positions.positions[index]];
            let kind = flat.scalar_type().ok_or(InspectError::Unsupported)?;
            budget.charge_allocation(1, size_of::<T>() as u64)?;
            members
                .try_reserve_exact(1)
                .map_err(|_| InspectError::Allocation)?;
            let collection = owned(owner.name(), budget)?;
            let field = Column::with_type(owned(name, budget)?, flat.column_type(kind)?, true)?;
            members.push(declaration(
                CollectionMember::new(collection, field)?,
                flat.text_sort_order(),
            ));
        }
    }
    check(budget, cancelled)?;
    Ok(members)
}
fn owned(value: &str, budget: &mut ResourceBudget) -> Result<String, InspectError> {
    budget.charge_allocation(value.len() as u64, 1)?;
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    output.push_str(value);
    Ok(output)
}
