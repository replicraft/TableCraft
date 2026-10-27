//! Re-derive unchanged composite topology and its source-seeded page framing.
use super::super::super::{
    composite::TupleProfile, encoding, plan::keys, profile::WriteProfile, tree::Tree,
};
use super::{AUDIT_CHUNK_BYTES, nonroot};
use crate::{
    EditError, InspectError, NativeImage, TableSchema,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::indexes::{
        NativeIndex,
        composite::{self, DecimalColumns, DecimalValue, MixedValue},
        header::IndexHeader,
        mutation::IndexKeyUpdate,
        page::{ENTRY_START, Entry, KeyRef},
    },
};
use std::io::Cursor;
use tablecraft_schema::ScalarType;

const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);
const MASK_START: usize = size_of::<IndexHeader>();

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub schema: &'a TableSchema,
    pub index: &'a NativeIndex,
    pub updates: &'a [IndexKeyUpdate],
}

pub(super) fn prove(
    operation: &Operation<'_>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let index = operation.index;
    let schema = operation.schema;

    actual.work(index.columns().len() * schema.columns.len())?;
    let has_guid = index.columns().iter().any(|key| {
        schema.columns.iter().any(|column| {
            column.number() == key.number() && column.scalar_type() == Some(ScalarType::Guid)
        })
    });

    if composite::has_decimal(index, schema)? {
        let profile = TupleProfile::<DecimalValue, DecimalColumns>::new_decimal(
            index,
            schema,
            actual.budget,
            actual.cancelled,
        )?;

        prepared::<_, { composite::MAX_GUID_ENTRY_BYTES }>(operation, &profile, actual)
    } else if has_guid {
        let profile =
            TupleProfile::<MixedValue>::new(index, schema, actual.budget, actual.cancelled)?;

        prepared::<_, { composite::MAX_GUID_ENTRY_BYTES }>(operation, &profile, actual)
    } else {
        let profile = TupleProfile::<i64>::new(index, schema, actual.budget, actual.cancelled)?;

        prepared::<_, { composite::MAX_ENTRY_BYTES }>(operation, &profile, actual)
    }
}

fn prepared<P: WriteProfile, const N: usize>(
    operation: &Operation<'_>,
    profile: &P,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let mut source = operation.image.reader();
    let prepared = keys::prepare::<P, N>(
        operation.image,
        &mut Reader {
            source: &mut source,
            header: operation.image.catalog().header(),
            budget: actual.budget,
            cancelled: actual.cancelled,
            reads: 0,
        },
        operation.index,
        operation.schema.definition,
        operation.updates,
        profile,
    )?;

    let mut replay = Replay {
        image: operation.image,
        profile,
        keys: &prepared.entries,
        position: 0,
    };

    replay.walk::<N>(&prepared.tree, actual)?;

    if replay.position != prepared.entries.len() {
        return Err(EditError::Preservation);
    }

    Ok(())
}

struct Replay<'a, P: WriteProfile> {
    image: &'a NativeImage,
    profile: &'a P,
    keys: &'a [KeyRef<P::Key>],
    position: usize,
}

impl<P: WriteProfile> Replay<'_, P> {
    fn walk<const N: usize>(
        &mut self,
        tree: &Tree,
        actual: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Option<usize>, EditError> {
        actual.tick()?;
        actual
            .budget
            .charge_allocation(tree.count() as u64, size_of::<Entry<&P::Key>>() as u64)
            .map_err(InspectError::from)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(tree.count())
            .map_err(|_| InspectError::Allocation)?;

        let maximum = if tree.is_leaf() {
            let end = self
                .position
                .checked_add(tree.count())
                .filter(|end| *end <= self.keys.len())
                .ok_or(EditError::Preservation)?;

            for key in &self.keys[self.position..end] {
                actual.tick()?;
                entries.push(Entry {
                    value: super::super::borrowed_reference(key),
                    child: 0,
                });
            }

            let maximum = (self.position < end).then(|| end - 1);
            self.position = end;
            maximum
        } else {
            let mut maximum = None;
            for (position, child) in tree.children().iter().enumerate() {
                actual.tick()?;
                let last = self
                    .walk::<N>(child, actual)?
                    .ok_or(EditError::Preservation)?;
                if position < tree.count() {
                    entries.push(Entry {
                        value: super::super::borrowed_reference(
                            self.keys.get(last).ok_or(EditError::Preservation)?,
                        ),
                        child: child.number(),
                    });
                }
                maximum = Some(last);
            }

            maximum
        };

        if entries.len() != tree.count() {
            return Err(EditError::Preservation);
        }

        let start = tree.number() as usize * PAGE_BYTES;
        let before = self
            .image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(InspectError::Malformed)?;
        let after = actual.page(tree.number())?;
        page::<P, N>(before, &after, &entries, self.profile, actual)?;

        Ok(maximum)
    }
}

fn page<P: WriteProfile, const N: usize>(
    before: &[u8],
    after: &[u8],
    entries: &[Entry<&P::Key>],
    profile: &P,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let original = IndexHeader::read(before)?;
    let end = nonroot::layout::<P, N>(
        entries,
        usize::from(original.prefix.get()),
        after,
        profile,
        actual,
    )?;

    let prefix = usize::from(IndexHeader::read(after)?.prefix.get());
    let mut position = ENTRY_START;
    for (ordinal, entry) in entries.iter().enumerate() {
        actual.tick()?;
        actual.work(N)?;
        let wire = encoding::borrowed_entry::<P, N>(entry, profile)?;
        let shared = if ordinal == 0 { 0 } else { prefix };
        let expected = wire
            .bytes
            .get(shared..wire.length)
            .ok_or(EditError::Preservation)?;
        let next = position
            .checked_add(expected.len())
            .filter(|next| *next <= end)
            .ok_or(EditError::Preservation)?;
        if after[position..next] != *expected {
            return Err(EditError::Preservation);
        }
        position = next;
    }

    if position != end {
        return Err(EditError::Preservation);
    }

    actual.work(PAGE_BYTES)?;
    for position in 0..PAGE_BYTES {
        if position.is_multiple_of(AUDIT_CHUNK_BYTES) {
            check(actual.budget, actual.cancelled)?;
        }
        let allowed = (FREE_OFFSET..FREE_OFFSET + size_of::<u16>()).contains(&position)
            || (PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()).contains(&position)
            || (MASK_START..end).contains(&position);
        if !allowed && before[position] != after[position] {
            return Err(EditError::Preservation);
        }
    }

    Ok(())
}
