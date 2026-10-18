use super::super::{
    IndexCheck, IndexCheckStatus,
    header::IndexHeader,
    page::{self, ENTRY_START, KeyRef},
    profile::KeyWidth,
};
use super::{IndexDataPatch, Int32KeyUpdate, encoding};
use crate::{
    EditError, FileFormat, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    column: usize,
    updates: &[Int32KeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    check(budget, cancelled)?;
    if image.catalog().header().format() != FileFormat::Jet4 {
        return Err(EditError::Unsupported(WriteBlocker::Format));
    }
    let mut patches = Vec::new();
    if updates.is_empty() {
        return Ok(patches);
    }
    if !indexes.iter().any(|index| {
        index
            .index()
            .columns()
            .iter()
            .any(|key| key.number() == schema.columns[column].number())
    }) {
        return Ok(patches);
    }
    // Every physical declaration must have a logical owner. Do not change a
    // live page also referenced by an uninspected/obsolete physical definition.
    for number in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, indexes.len() as u64)
            .map_err(InspectError::from)?;
        if !indexes
            .iter()
            .any(|index| index.index().data_number() == number as u32)
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    budget
        .charge_allocation(indexes.len() as u64, size_of::<IndexDataPatch>() as u64)
        .map_err(InspectError::from)?;
    patches
        .try_reserve_exact(indexes.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for (position, check) in indexes.iter().enumerate() {
        reader.tick()?;
        let index = check.index();
        if !index
            .columns()
            .iter()
            .any(|key| key.number() == schema.columns[column].number())
        {
            continue;
        }
        if check.status() != IndexCheckStatus::Checked
            || index.columns().len() != 1
            || !index
                .constraints()
                .is_some_and(|constraints| constraints.unique() && !constraints.ignore_nulls())
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        reader.work(indexes.len())?;
        if indexes.iter().any(|other| {
            other.index().data_number() != index.data_number()
                && other.index().root_page() == index.root_page()
        }) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if indexes[..position]
            .iter()
            .any(|previous| previous.index().data_number() == index.data_number())
        {
            continue;
        }
        let descending = index.columns()[0].descending();
        let page = page::read(
            &mut reader,
            index.root_page(),
            schema.definition,
            descending,
            KeyWidth::Int32,
        )?;
        if !page.leaf || page.previous != 0 || page.next != 0 || page.tail != 0 {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let offset = index.root_page() as usize * PAGE_BYTES;
        let raw = image
            .bytes
            .get(offset..offset + PAGE_BYTES)
            .ok_or(InspectError::Malformed)?;
        let header = IndexHeader::read(raw)?;
        if !header.qualified_for_leaf_update() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let end = PAGE_BYTES
            .checked_sub(usize::from(header.free.get()))
            .ok_or(InspectError::Malformed)?;
        if end < ENTRY_START {
            return Err(InspectError::Malformed.into());
        }
        reader
            .budget
            .charge_allocation(page.entries.len() as u64, size_of::<KeyRef>() as u64)
            .map_err(InspectError::from)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(page.entries.len())
            .map_err(|_| InspectError::Allocation)?;
        let mut matched = 0;
        for entry in page.entries {
            reader.tick()?;
            reader.work(updates.len())?;
            let mut key = entry.value;
            if let Some(update) = updates.iter().find(|update| update.reference == key.row) {
                if key.key != Some(i64::from(update.expected)) {
                    return Err(EditError::Conflict);
                }
                key.key = Some(i64::from(update.value));
                matched += 1;
            }
            entries.push(key);
        }
        if matched != updates.len() {
            return Err(EditError::Preservation);
        }
        let levels = entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
        reader.work(entries.len() * levels * size_of::<KeyRef>())?;
        entries.sort_unstable_by(|a, b| a.compare(*b, descending));
        let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
        let mut previous = None;
        for entry in &entries {
            reader.tick()?;
            constraints
                .check_key(&mut previous, entry.key)
                .map_err(InspectError::from)?;
        }
        let after = encoding::encode(
            &entries,
            usize::from(header.prefix.get()),
            end - ENTRY_START,
            descending,
            reader.budget,
            cancelled,
        )?;
        reader
            .budget
            .charge_allocation((end - ENTRY_START) as u64, 1)
            .map_err(InspectError::from)?;
        let mut before = Vec::new();
        before
            .try_reserve_exact(end - ENTRY_START)
            .map_err(|_| InspectError::Allocation)?;
        before.extend_from_slice(&raw[ENTRY_START..end]);
        patches.push(IndexDataPatch {
            offset: offset + ENTRY_START,
            before,
            after,
        });
    }
    check(budget, cancelled)?;
    Ok(patches)
}
