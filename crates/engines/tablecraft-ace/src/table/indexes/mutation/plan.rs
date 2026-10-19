use super::super::{IndexCheck, IndexCheckStatus};
use super::{IndexDataPatch, Int32KeyUpdate, tree::Tree};
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker, format::reader::Reader,
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
    // Format admission belongs to the public edit/preview entry points. This
    // private helper receives only already checked schemas/index coverage.
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
        let mut entries = Vec::new();
        let tree = Tree::read(
            image,
            &mut reader,
            schema.definition,
            index.root_page(),
            descending,
            0,
            &mut entries,
        )?;
        let mut matched = 0;
        for key in &mut entries {
            reader.tick()?;
            reader.work(updates.len())?;
            if let Some(update) = updates.iter().find(|update| update.reference == key.row) {
                if key.key != Some(i64::from(update.expected)) {
                    return Err(EditError::Conflict);
                }
                key.key = Some(i64::from(update.value));
                matched += 1;
            }
        }
        if matched != updates.len() {
            return Err(EditError::Preservation);
        }
        let levels = entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
        reader.work(entries.len() * levels * size_of::<super::super::page::KeyRef>())?;
        entries.sort_unstable_by(|a, b| a.compare(*b, descending));
        let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
        let mut previous = None;
        for entry in &entries {
            reader.tick()?;
            constraints
                .check_key(&mut previous, entry.key)
                .map_err(InspectError::from)?;
        }
        let mut remaining = entries.as_slice();
        tree.rewrite(
            image,
            &mut remaining,
            descending,
            &mut patches,
            reader.budget,
            cancelled,
        )?;
        if !remaining.is_empty() {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(patches)
}
