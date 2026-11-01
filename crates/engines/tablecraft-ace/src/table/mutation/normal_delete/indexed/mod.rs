//! Complete original single-I32 trees, aliases and exclusive mutation roles.
mod proof;

use crate::{
    EditError, InspectError, NativeImage, RowDelete, TableCheck, WriteBlocker,
    format::{data_type::INT32, page::PAGE_BYTES},
    header::check,
    table::{
        indexes::{
            header::IndexHeader,
            mutation::{IndexDataPatch, normal_deletion, normal_index_audit::Ownership},
            page::{LEAF, NODE},
        },
        mutation::{
            fixed_records::indexed::CheckedIndexes, lval_ownership::Inventory,
            preserved_source::Source,
        },
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Admission<'image> {
    facts: CheckedIndexes<'image>,
    ownership: Ownership,
}

impl<'image> Admission<'image> {
    fn capture(
        image: &'image NativeImage,
        source: &Source,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let facts =
            CheckedIndexes::capture(image, source.table.schema().name(), budget, cancelled)?;
        facts.require_source(image, source.table.schema(), budget, cancelled)?;
        let checked = facts.checked();
        let schema = checked.schema();
        for column in source.table.columns() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, column.name().len() as u64)
                .map_err(InspectError::from)?;
        }
        if checked.rows() != source.records.len()
            || source.table.rows().len() != source.records.len()
            || schema.scalar_columns_with_budget(budget)? != source.table.columns()
            || checked.indexes().len() != source.indexes.indexes().len()
        {
            return Err(EditError::Preservation);
        }
        let mut has_node = false;
        let mut pages = vector(checked.indexes().len(), budget, cancelled)?;
        for (position, check_index) in checked.indexes().iter().enumerate() {
            check(budget, cancelled)?;
            let index = check_index.index();
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            if index.foreign_reference().is_some()
                || !index
                    .constraints()
                    .is_some_and(|constraints| !constraints.ignore_nulls())
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            budget
                .charge(Resource::Steps, schema.columns().len() as u64)
                .map_err(InspectError::from)?;
            let field = schema
                .columns()
                .iter()
                .find(|field| field.number() == key.number())
                .ok_or(InspectError::Malformed)?;
            if field.native_type() != INT32 {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            budget
                .charge(Resource::Steps, position as u64)
                .map_err(InspectError::from)?;
            if checked.indexes()[..position]
                .iter()
                .any(|prior| prior.index().data_number() == index.data_number())
            {
                continue;
            }
            let start = (index.root_page() as usize)
                .checked_mul(PAGE_BYTES)
                .ok_or(InspectError::StructureLimit)?;
            let end = start
                .checked_add(PAGE_BYTES)
                .ok_or(InspectError::StructureLimit)?;
            let raw = image
                .as_bytes()
                .get(start..end)
                .ok_or(InspectError::Truncated)?;
            let header = IndexHeader::read(raw)?;
            if !matches!(header.kind, LEAF | NODE)
                || header.previous.get() != 0
                || header.next.get() != 0
                || (header.kind == LEAF
                    && (header.tail.get() != 0 || !header.qualified_for_update(0)))
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            has_node |= header.kind == NODE;
            pages.push(index.root_page());
        }
        // This is actual catalog-wide ownership, not a selected map-only test.
        let ownership = Ownership::capture(image, checked, budget, cancelled)?;
        for check_index in checked.indexes() {
            ownership.require(image, schema, check_index.index(), budget, cancelled)?;
        }
        if has_node {
            // Whole selected INDEX graphs, not just NODE roots, become mutable.
            // Keep the old leaf-only pool/poll path unchanged.
            pages = tree_pages(image, checked, &ownership, budget, cancelled)?;
        }
        let inventory = Inventory::capture(image, budget, cancelled)?;
        inventory.require_no_long_value_data_overlap(&pages, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self { facts, ownership })
    }
}

fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}

pub(super) fn qualify(
    image: &NativeImage,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    Admission::capture(image, source, budget, cancelled)?;
    Ok(())
}

fn references(
    source: &Source,
    deletes: &[RowDelete],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    let mut references = vector(deletes.len(), budget, cancelled)?;
    for request in deletes {
        check(budget, cancelled)?;
        let record = source
            .records
            .get(request.row())
            .ok_or(EditError::Conflict)?;
        budget
            .charge(Resource::Steps, record.data.len() as u64)
            .map_err(InspectError::from)?;
        if record.data != request.expected_bytes() {
            return Err(EditError::Conflict);
        }
        references.push(record.reference);
    }
    Ok(references)
}

pub(super) fn patches(
    image: &NativeImage,
    source: &Source,
    deletes: &[RowDelete],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    let admission = Admission::capture(image, source, budget, cancelled)?;
    let references = references(source, deletes, budget, cancelled)?;
    normal_deletion::patches(
        image,
        admission.facts.checked(),
        &references,
        budget,
        cancelled,
    )
}

pub(super) use proof::reconstruct;

fn tree_pages(
    image: &NativeImage,
    checked: &TableCheck,
    ownership: &Ownership,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    let mut count = 0_usize;
    for (position, entry) in checked.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        let index = entry.index();
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        let tree = ownership.require(image, checked.schema(), index, budget, cancelled)?;
        count = count
            .checked_add(tree.pages().len())
            .ok_or(InspectError::StructureLimit)?;
    }
    let mut pages = vector(count, budget, cancelled)?;
    for (position, entry) in checked.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        let index = entry.index();
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        let tree = ownership.require(image, checked.schema(), index, budget, cancelled)?;
        for page in tree.pages() {
            check(budget, cancelled)?;
            pages.push(*page);
        }
    }
    Ok(pages)
}
