//! Explicit bounded reciprocal-link inspection; no recursive graph traversal.
use super::{
    IndexInspection, NativeIndex, catalog::Relationships, inspect_table_indexes,
    reference::FOREIGN_INDEX,
};
use crate::{
    CatalogInspection, InspectError, TableSchema, format::page::PAGE_NUMBER_MASK, header::check,
    inspect_catalog,
};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};
use tablecraft_model::CatalogScope;

const LOCAL_TABLE: i16 = 1;

/// One reciprocal pair checked against its enforced catalog declaration.
/// This does not prove key membership or safe cascade/write behavior.
pub struct ForeignIndexLink {
    index: u32,
    table: ObjectId,
    target: NativeIndex,
}
impl ForeignIndexLink {
    /// Logical index number on the explicitly inspected source table.
    pub const fn index_number(&self) -> u32 {
        self.index
    }
    /// Catalog-scoped identity resolved from the referenced definition page.
    pub const fn referenced_table(&self) -> ObjectId {
        self.table
    }
    /// Referenced logical index, including its reciprocal pointer.
    pub fn referenced_index(&self) -> &NativeIndex {
        &self.target
    }
}
/// Complete metadata for one explicit table and all its reciprocal foreign links.
/// Reads system relationship metadata, not user records or active content.
pub struct ForeignIndexInspection {
    inspection: IndexInspection,
    links: Vec<ForeignIndexLink>,
}
impl ForeignIndexInspection {
    /// Owning table metadata.
    pub fn schema(&self) -> &TableSchema {
        self.inspection.schema()
    }
    /// All local logical indexes in native order, including ordinary aliases.
    pub fn indexes(&self) -> &[NativeIndex] {
        self.inspection.indexes()
    }
    /// Checked reciprocal links, in local native index order.
    pub fn links(&self) -> &[ForeignIndexLink] {
        &self.links
    }
}

/// Inspect both ends of each foreign-index link on a stable/offline source.
/// Resolve unique local table identities and logical index numbers, opposite
/// roles, matching actions, key counts and reciprocal pointers. Check complete
/// enforced relationship-catalog coverage, aligned key-column pairs and declared
/// parent/one-to-one uniqueness. Unknown roles/
/// flags or a missing reference are unsupported; broken pointers are malformed.
/// No recursion, user-row reads, provider activation or partial report is permitted.
pub fn inspect_foreign_index_links<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ForeignIndexInspection, InspectError> {
    let _span = tracing::debug_span!("native.index.foreign_links").entered();
    let inspection = inspect_table_indexes(source, scope, table, budget, cancelled)?;
    let links = validate(source, scope, &inspection, budget, cancelled)?;
    Ok(ForeignIndexInspection { inspection, links })
}

// Qualify declarations only after the complete metadata operation succeeds.
// Unsupported validation leaves every original raw declaration unqualified.
pub(super) fn qualify<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    inspection: &mut IndexInspection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    if !inspection
        .indexes
        .iter()
        .any(|i| i.native_type() == FOREIGN_INDEX)
    {
        return Ok(());
    }
    validate(source, scope, inspection, budget, cancelled)?;
    for index in &mut inspection.indexes {
        index.foreign_checked = index.native_type() == FOREIGN_INDEX;
    }
    Ok(())
}

fn validate<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    inspection: &IndexInspection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<ForeignIndexLink>, InspectError> {
    let mut relationships = Relationships::read(source, scope, budget, cancelled)?;
    let count = inspection
        .indexes
        .iter()
        .filter(|i| i.native_type() == FOREIGN_INDEX)
        .count();
    budget.charge_allocation(count as u64, size_of::<ForeignIndexLink>() as u64)?;
    let mut links = Vec::new();
    links
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    if count != 0 {
        let catalog = inspect_catalog(source, scope, budget, cancelled)?;
        if local_table(
            &catalog,
            inspection.schema().definition_page(),
            budget,
            cancelled,
        )? != inspection.schema().object_id()
        {
            return Err(InspectError::Malformed);
        }
        for index in &inspection.indexes {
            check(budget, cancelled)?;
            if index.native_type() != FOREIGN_INDEX {
                continue;
            }
            let reference = index.foreign_reference().ok_or(InspectError::Unsupported)?;
            reference.qualify()?;
            if reference.table_page() == 0
                || reference.table_page() >= catalog.header().page_count()
            {
                return Err(InspectError::Malformed);
            }
            let id = local_table(&catalog, reference.table_page(), budget, cancelled)?;
            let object = catalog.catalog().get(id).ok_or(InspectError::Malformed)?;
            let mut other =
                inspect_table_indexes(source, scope, object.name().as_str(), budget, cancelled)?;
            if other.schema().object_id() != id
                || other.schema().definition_page() != reference.table_page()
            {
                return Err(InspectError::Malformed);
            }
            let mut target_position = None;
            for (position, candidate) in other.indexes.iter().enumerate() {
                check(budget, cancelled)?;
                if candidate.number() == reference.index_number() {
                    target_position = Some(position);
                }
            }
            let target_position = target_position.ok_or(InspectError::Malformed)?;
            let target = &other.indexes[target_position];
            let back = target.foreign_reference().ok_or(InspectError::Malformed)?;
            back.qualify()?;
            if !reference.reciprocal(back, inspection.schema().definition_page(), index.number())
                || index.columns().len() != target.columns().len()
            {
                return Err(InspectError::Malformed);
            }
            relationships.link(
                (&inspection.schema, index, &inspection.indexes),
                (&other.schema, target, &other.indexes),
                budget,
                cancelled,
            )?;
            links.push(ForeignIndexLink {
                index: index.number(),
                table: id,
                target: other.indexes.remove(target_position),
            });
        }
    }
    relationships.finish(inspection.schema().name(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(links)
}

fn local_table(
    catalog: &CatalogInspection,
    page: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ObjectId, InspectError> {
    let mut target = None;
    for origin in catalog.origins() {
        check(budget, cancelled)?;
        if origin.type_code() == LOCAL_TABLE
            && origin.native_id() as u32 & PAGE_NUMBER_MASK == page
            && target.replace(origin.object_id()).is_some()
        {
            return Err(InspectError::Malformed);
        }
    }
    target.ok_or(InspectError::Malformed)
}
