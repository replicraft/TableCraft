//! Explicit bounded reciprocal-link inspection; no recursive graph traversal.
use super::{IndexInspection, NativeIndex, inspect_table_indexes, reference::FOREIGN_INDEX};
use crate::{
    InspectError, TableSchema, format::page::PAGE_NUMBER_MASK, header::check, inspect_catalog,
};
use std::io::{Read, Seek};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};
use tablecraft_model::CatalogScope;

const LOCAL_TABLE: i16 = 1;

/// One checked reciprocal pair. This proves pointer agreement only, not key
/// membership, relationship-catalog agreement or safe cascade/write behavior.
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
/// Does not read table records or execute active content or cascade actions.
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
/// roles, matching actions, key counts and reciprocal pointers. Unknown roles/
/// flags or a missing reference are unsupported; broken pointers are malformed.
/// No recursion, row reads, provider activation or partial report is permitted.
pub fn inspect_foreign_index_links<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ForeignIndexInspection, InspectError> {
    let _span = tracing::debug_span!("native.index.foreign_links").entered();
    let inspection = inspect_table_indexes(source, scope, table, budget, cancelled)?;
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
            let mut target = None;
            for origin in catalog.origins() {
                check(budget, cancelled)?;
                if origin.type_code() == LOCAL_TABLE
                    && origin.native_id() as u32 & PAGE_NUMBER_MASK == reference.table_page()
                    && target.replace(origin.object_id()).is_some()
                {
                    return Err(InspectError::Malformed);
                }
            }
            let id = target.ok_or(InspectError::Malformed)?;
            let object = catalog.catalog().get(id).ok_or(InspectError::Malformed)?;
            let other =
                inspect_table_indexes(source, scope, object.name().as_str(), budget, cancelled)?;
            if other.schema().object_id() != id
                || other.schema().definition_page() != reference.table_page()
            {
                return Err(InspectError::Malformed);
            }
            let mut target = None;
            for candidate in other.indexes {
                check(budget, cancelled)?;
                if candidate.number() == reference.index_number() {
                    target = Some(candidate);
                }
            }
            let target = target.ok_or(InspectError::Malformed)?;
            let back = target.foreign_reference().ok_or(InspectError::Malformed)?;
            back.qualify()?;
            if !reference.reciprocal(back, inspection.schema().definition_page(), index.number())
                || index.columns().len() != target.columns().len()
            {
                return Err(InspectError::Malformed);
            }
            links.push(ForeignIndexLink {
                index: index.number(),
                table: id,
                target,
            });
        }
    }
    check(budget, cancelled)?;
    Ok(ForeignIndexInspection { inspection, links })
}
