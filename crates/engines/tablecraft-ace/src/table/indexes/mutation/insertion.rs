//! Request membership and cardinality growth for one qualified Int32 primary.
mod height_one;
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
    size_of::<u8>() + size_of::<i32>() + size_of::<u32>() + size_of::<u32>();

/// Borrow the request carrier. DATA certification must independently establish
/// the new logical ROW; this key planner does not grant DATA placement authority.
pub(in crate::table) struct InsertKey<'a> {
    pub reference: u32,
    pub value: &'a Value,
}

/// Builder comparison facts, not a sealed request or after-image certificate.
pub(in crate::table) struct PrimaryInsertion {
    pub patches: Vec<IndexDataPatch>,
    pub added: Vec<AddedPage>,
    pub promotions: Vec<Promotion>,
    pub rebases: Vec<Rebase>,
    requires_split: bool,
    physical: u32,
    root: u32,
    usage: u32,
    original_count: usize,
    expected: Vec<KeyRef>,
}

impl PrimaryInsertion {
    /// Transfer the DATA cursor through index allocation and return its sole
    /// final owner. A fitted leaf retains the exact existing serialization.
    pub(in crate::table) fn continue_with_append(
        mut self,
        image: &NativeImage,
        checked: &TableCheck,
        id: usize,
        append: State,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, State), EditError> {
        check(budget, cancelled)?;
        if !self.requires_split {
            return Ok((self, append));
        }

        let plan = split::insert::continue_primary(
            split::insert::Operation {
                image,
                checked,
                id,
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
        self.requires_split = false;

        Ok((self, plan.append))
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
    requests: &[InsertKey<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PrimaryInsertion, EditError> {
    check(budget, cancelled)?;
    let index = qualify(checked, id_column, budget, cancelled)?;
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
    let source_height_one =
        header.kind == NODE && header.tail.get() != 0 && header.qualified_for_update(1);
    if (!source_leaf && !source_height_one) || header.previous.get() != 0 || header.next.get() != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let profile = SingleProfile {
        column: schema.columns()[id_column].number(),
        kind: NumericKey::new(ScalarType::Int32)?,
        descending: false,
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
        || (source_height_one && tree.height() != 1)
    {
        return Err(EditError::Preservation);
    }
    let original_count = expected.len();
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
        let value = match request.value {
            Value::Int32(value) => i64::from(*value),
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        };
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
            key: Some(value),
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
    let mut requires_split = false;
    if source_height_one {
        patches = height_one::prepare(
            image,
            checked,
            id_column,
            &expected,
            reader.budget,
            cancelled,
        )?;
    } else if !requests.is_empty() {
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
            requires_split = true;
        }
    }
    check(reader.budget, cancelled)?;
    Ok(PrimaryInsertion {
        patches,
        added: Vec::new(),
        promotions: Vec::new(),
        rebases: Vec::new(),
        requires_split,
        physical: index.data_number(),
        root: index.root_page(),
        usage: index.usage_map(),
        original_count,
        expected,
    })
}

fn qualify<'a>(
    checked: &'a TableCheck,
    id_column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeIndex, EditError> {
    let schema = checked.schema();
    let field = schema
        .columns()
        .get(id_column)
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    if field.scalar_type() != Some(ScalarType::Int32) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    if schema.layout.physical_indexes != 1 || checked.indexes().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut primary = None;
    let first = checked.indexes()[0].index();
    for checked_index in checked.indexes() {
        check(budget, cancelled)?;
        let index = checked_index.index();
        let constraints = index
            .constraints()
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        if checked_index.status() != IndexCheckStatus::Checked
            || index.native_type() > PRIMARY_INDEX
            || index.data_number() != 0
            || index.root_page() != first.root_page()
            || index.usage_map() != first.usage_map()
            || index.flags() != first.flags()
            || index.columns() != first.columns()
            || index.columns().len() != 1
            || index.columns()[0].number() != field.number()
            || index.columns()[0].descending()
            || !constraints.unique()
            || !constraints.required()
            || constraints.ignore_nulls()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if constraints.primary_key() && primary.replace(index).is_some() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    primary.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))
}

#[cfg(test)]
mod tests;
