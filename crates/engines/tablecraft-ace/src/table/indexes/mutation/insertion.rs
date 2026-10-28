//! Request membership and cardinality growth for one qualified physical numeric index.
use super::split::{
    self, AddedPage,
    allocation::{Promotion, Rebase},
};
use super::{IndexDataPatch, NumericKey, profile::SingleProfile, repack, tree::Tree};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    format::{
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, reference_page, reference_slot},
    },
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_LIVE_ROWS},
    table::indexes::{
        constraints::PRIMARY_INDEX,
        header::IndexHeader,
        page::{KeyRef, LEAF, NODE},
    },
    table::mutation::append::State,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

const ENTRY_BUFFER: usize =
    size_of::<u8>() + size_of::<i64>() + size_of::<u32>() + size_of::<u32>();

/// Borrow the request carrier. DATA certification must independently establish
/// the new logical ROW; this key planner does not grant DATA placement authority.
pub(in crate::table) struct InsertKey<'a> {
    pub reference: u32,
    pub value: &'a Value,
}

/// Builder comparison facts, not a sealed request or after-image certificate.
pub(in crate::table) struct IndexInsertion {
    pub patches: Vec<IndexDataPatch>,
    pub added: Vec<AddedPage>,
    pub promotions: Vec<Promotion>,
    pub rebases: Vec<Rebase>,
    requires_continuation: bool,
    physical: u32,
    root: u32,
    usage: u32,
    original_count: usize,
    source_pages: Vec<u32>,
    column: usize,
    distinct_added: usize,
    expected: Vec<KeyRef>,
}

impl IndexInsertion {
    /// Transfer the DATA cursor through physical-index maintenance and return its sole
    /// final owner. Bounded root maxima can change without allocating pages.
    pub(in crate::table) fn continue_with_append(
        mut self,
        image: &NativeImage,
        checked: &TableCheck,
        append: State,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, State), EditError> {
        check(budget, cancelled)?;
        if !self.requires_continuation {
            return Ok((self, append));
        }

        let plan = split::insert::continue_primary(
            split::insert::Operation {
                image,
                checked,
                id: self.column,
                physical: self.physical,
                final_keys: &self.expected,
            },
            append,
            budget,
            cancelled,
        )?;
        self.patches = plan.patches;
        self.added = plan.added;
        self.promotions = plan.promotions;
        self.rebases = plan.rebases;
        self.requires_continuation = false;

        Ok((self, plan.append))
    }

    pub(in crate::table) fn source_pages(&self) -> &[u32] {
        &self.source_pages
    }

    pub(in crate::table) fn physical(&self) -> u32 {
        self.physical
    }
    pub(in crate::table) fn root(&self) -> u32 {
        self.root
    }
    pub(in crate::table) fn usage(&self) -> u32 {
        self.usage
    }
    pub(in crate::table) fn distinct_added(&self) -> usize {
        self.distinct_added
    }

    pub(in crate::table) fn original_count(&self) -> usize {
        self.original_count
    }
    pub(in crate::table) fn expected(
        &self,
    ) -> impl ExactSizeIterator<Item = (Option<i64>, u32)> + '_ {
        self.expected.iter().map(|entry| (entry.key, entry.row))
    }
}

pub(in crate::table) fn prepare(
    image: &NativeImage,
    checked: &TableCheck,
    id_column: usize,
    physical: u32,
    requests: &[InsertKey<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexInsertion, EditError> {
    check(budget, cancelled)?;
    let index = qualify(checked, id_column, physical, budget, cancelled)?;
    let schema = checked.schema();
    let root_start = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let original = image
        .as_bytes()
        .get(root_start..root_start + PAGE_BYTES)
        .ok_or(InspectError::Truncated)?;
    let header = IndexHeader::read(original)?;
    let source_leaf =
        header.kind == LEAF && header.tail.get() == 0 && header.qualified_for_update(0);
    let source_node = header.kind == NODE;
    if (!source_leaf && !source_node) || header.previous.get() != 0 || header.next.get() != 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let profile = SingleProfile {
        column: schema.columns()[id_column].number(),
        kind: NumericKey::new(
            schema.columns()[id_column]
                .scalar_type()
                .ok_or(InspectError::Unsupported)?,
        )?,
        descending: index.columns()[0].descending(),
    };
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut expected = Vec::new();
    let tree = Tree::read::<_, ENTRY_BUFFER>(
        image,
        &mut reader,
        schema.definition_page(),
        index.root_page(),
        &profile,
        0,
        &mut expected,
    )?;
    if expected.len() != checked.rows()
        || (source_leaf && !tree.is_leaf())
        || (source_node && tree.is_leaf())
    {
        return Err(EditError::Preservation);
    }
    let mut source_pages = Vec::new();
    collect_pages(&tree, &mut source_pages, &mut reader)?;
    let original_count = expected.len();
    reader.work(expected.len())?;
    let original_distinct = distinct(&expected);
    let final_count = original_count
        .checked_add(requests.len())
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    reader
        .budget
        .charge_allocation(requests.len() as u64, size_of::<KeyRef>() as u64)
        .map_err(InspectError::from)?;
    expected
        .try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;
    for request in requests {
        reader.tick()?;
        let value = profile.kind.key_type().key(request.value)?;
        let page = reference_page(request.reference);
        if page == 0
            || u64::from(page) >= MAX_IMAGE_BYTES / PAGE_BYTES as u64
            || usize::from(reference_slot(request.reference)) >= MAX_SLOTS
        {
            return Err(EditError::Conflict);
        }
        reader.work(expected.len())?;
        if expected.iter().any(|entry| entry.row == request.reference) {
            return Err(EditError::Conflict);
        }
        expected.push(KeyRef {
            key: value,
            row: request.reference,
        });
    }
    if expected.len() != final_count {
        return Err(EditError::Preservation);
    }
    let levels = final_count.checked_ilog2().unwrap_or(0) as usize + 1;
    let sort_work = final_count
        .checked_mul(levels)
        .and_then(|work| work.checked_mul(size_of::<KeyRef>()))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(sort_work)?;
    expected.sort_unstable_by(|left, right| left.compare_with(right, &profile));
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let mut previous = None;
    for entry in &expected {
        reader.tick()?;
        constraints
            .check_tuple(&mut previous, entry.key, entry.key.is_none())
            .map_err(InspectError::from)?;
    }
    let mut patches = Vec::new();
    // NODE continuation also rebuilds bounded parent maxima when every leaf
    // fits; all NODE sources use that same owned service.
    let mut requires_continuation = source_node && !requests.is_empty();
    if !source_node && !requests.is_empty() {
        let entries = split::leaf_entries(&expected, reader.budget, cancelled)?;
        if repack::borrowed_fits::<_, ENTRY_BUFFER>(
            original,
            &entries,
            &profile,
            reader.budget,
            cancelled,
        )? {
            let after = repack::borrowed_page::<_, ENTRY_BUFFER>(
                original,
                &entries,
                &profile,
                reader.budget,
                cancelled,
            )?;
            repack::patches(
                original,
                &after,
                index.root_page(),
                &mut patches,
                reader.budget,
                cancelled,
            )?;
        } else {
            requires_continuation = true;
        }
    }
    check(reader.budget, cancelled)?;
    reader.work(expected.len())?;
    let distinct_added = distinct(&expected)
        .checked_sub(original_distinct)
        .ok_or(EditError::Preservation)?;
    Ok(IndexInsertion {
        patches,
        added: Vec::new(),
        promotions: Vec::new(),
        rebases: Vec::new(),
        requires_continuation,
        physical: index.data_number(),
        root: index.root_page(),
        usage: index.usage_map(),
        original_count,
        source_pages,
        column: id_column,
        distinct_added,
        expected,
    })
}

pub(in crate::table::indexes) fn qualify<'a>(
    checked: &'a TableCheck,
    column: usize,
    physical: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeIndex, EditError> {
    let schema = checked.schema();
    let field = schema
        .columns()
        .get(column)
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    if !matches!(
        field.scalar_type(),
        Some(
            ScalarType::Byte
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Currency
        )
    ) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    if physical as usize >= schema.layout.physical_indexes {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let first = checked
        .indexes()
        .iter()
        .find(|item| item.index().data_number() == physical)
        .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?
        .index();
    let mut primary = false;
    for item in checked
        .indexes()
        .iter()
        .filter(|item| item.index().data_number() == physical)
    {
        check(budget, cancelled)?;
        let index = item.index();
        let constraints = index
            .constraints()
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        if item.status() != IndexCheckStatus::Checked
            || index.native_type() > PRIMARY_INDEX
            || index.root_page() != first.root_page()
            || index.usage_map() != first.usage_map()
            || index.flags() != first.flags()
            || index.columns() != first.columns()
            || index.columns().len() != 1
            || index.columns()[0].number() != field.number()
            || constraints.ignore_nulls()
            || (constraints.primary_key()
                && (primary
                    || field.scalar_type() != Some(ScalarType::Int32)
                    || index.columns()[0].descending()
                    || !constraints.unique()
                    || !constraints.required()))
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        primary |= constraints.primary_key();
    }
    Ok(first)
}

fn collect_pages(
    tree: &Tree,
    pages: &mut Vec<u32>,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    reader
        .budget
        .charge_allocation(1, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    pages
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)?;
    pages.push(tree.number());
    for child in tree.children() {
        collect_pages(child, pages, reader)?;
    }
    Ok(())
}

fn distinct(entries: &[KeyRef]) -> usize {
    entries
        .iter()
        .enumerate()
        .filter(|(position, entry)| *position == 0 || entries[*position - 1].key != entry.key)
        .count()
}

#[cfg(test)]
mod tests;
