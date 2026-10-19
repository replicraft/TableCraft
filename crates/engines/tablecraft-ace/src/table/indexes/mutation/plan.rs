use super::super::{IndexCheck, IndexCheckStatus, NativeIndex, composite, page::KeyRef};
use super::{
    IndexDataPatch, IntegralKey, IntegralKeyUpdate,
    composite::TupleProfile,
    encoding::{self, Mode},
    profile::{SingleProfile, WriteProfile},
    tree::{Rewrite, Tree},
};
use crate::{
    EditError, FileFormat, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::reader::Reader, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    updates: &[IntegralKeyUpdate],
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
        reader.work(updates.len() * index.columns().len())?;
        if !updates.iter().any(|update| {
            index
                .columns()
                .iter()
                .any(|key| key.number() == schema.columns[update.column].number())
        }) {
            continue;
        }
        if check.status() != IndexCheckStatus::Checked
            || !index
                .constraints()
                .is_some_and(|constraints| !constraints.ignore_nulls())
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
        if index.columns().len() == 1 {
            let column = schema
                .columns
                .iter()
                .position(|column| column.number() == index.columns()[0].number())
                .ok_or(InspectError::Malformed)?;
            let kind = IntegralKey::new(
                schema.columns[column]
                    .scalar_type()
                    .ok_or(InspectError::Unsupported)?,
            )?;
            let profile = SingleProfile {
                column,
                kind,
                descending: index.columns()[0].descending(),
            };
            edit::<_, { encoding::MAX_ENTRY_BYTES }>(
                image,
                &mut reader,
                index,
                schema.definition,
                updates,
                &profile,
                &mut patches,
            )?;
        } else {
            if !matches!(
                image.catalog().header().format(),
                FileFormat::Ace2016 | FileFormat::Ace2019
            ) {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            let profile = TupleProfile::new(index, schema, reader.budget, reader.cancelled)?;
            edit::<_, { composite::MAX_ENTRY_BYTES }>(
                image,
                &mut reader,
                index,
                schema.definition,
                updates,
                &profile,
                &mut patches,
            )?;
        }
    }
    check(budget, cancelled)?;
    Ok(patches)
}

fn edit<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    updates: &[IntegralKeyUpdate],
    profile: &P,
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    let mut entries = Vec::new();
    let tree = Tree::read::<_, N>(
        image,
        reader,
        owner,
        index.root_page(),
        profile,
        0,
        &mut entries,
    )?;
    let mut matched = 0;
    for key in &mut entries {
        reader.tick()?;
        reader.work(updates.len())?;
        for update in updates
            .iter()
            .filter(|update| update.reference == key.row && profile.includes(update.column))
        {
            profile.update(&mut key.key, update.column, update.expected, update.value)?;
            matched += 1;
        }
    }
    reader.work(updates.len() * profile.components())?;
    if matched
        != updates
            .iter()
            .filter(|update| profile.includes(update.column))
            .count()
    {
        return Err(EditError::Preservation);
    }
    let levels = entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(entries.len() * levels * size_of::<KeyRef<P::Key>>())?;
    entries.sort_unstable_by(|a, b| a.compare_with(*b, profile));
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let mut previous = None;
    for entry in &entries {
        reader.tick()?;
        constraints
            .check_tuple(
                &mut previous,
                if profile.all_null(entry.key) {
                    None
                } else {
                    Some(entry.key)
                },
                profile.any_null(entry.key),
            )
            .map_err(InspectError::from)?;
    }
    let mode = if profile.components() > 1
        || updates
            .iter()
            .any(|u| profile.includes(u.column) && (u.expected.is_none() || u.value.is_none()))
    {
        Mode::Repack
    } else {
        Mode::Fixed
    };
    let mut remaining = entries.as_slice();
    tree.rewrite::<_, N>(
        image,
        &mut remaining,
        &Rewrite { profile, mode },
        patches,
        reader.budget,
        reader.cancelled,
    )?;
    if !remaining.is_empty() {
        return Err(EditError::Preservation);
    }
    Ok(())
}
