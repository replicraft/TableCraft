//! Independent complete-row population and shared-EOF replay for fixed batches.
mod history;
mod population;
mod source;

#[cfg(test)]
mod tests;

use super::super::{
    super::{
        IndexKeyUpdate, NumericKey,
        composite::TupleProfile,
        encoding,
        profile::{SingleProfile, WriteProfile},
    },
    Plan,
    fixed_batch::Input,
};
use super::{maps, retained};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    inspect_header,
    limits::MAX_IMAGE_BYTES,
    table::{
        indexes::composite,
        mutation::{append::owners::OwnerContext, map_authority},
    },
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(in crate::table::indexes::mutation::split) fn prove<O: OwnerContext>(
    image: &NativeImage,
    inputs: &[Input<'_>],
    plan: &Plan<O>,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Range<usize>>, EditError> {
    check(budget, cancelled)?;
    if plan.append.original_length != image.as_bytes().len()
        || plan.append.length != after.len()
        || after.len() < image.as_bytes().len()
        || after.len() as u64 > MAX_IMAGE_BYTES
        || !after.len().is_multiple_of(PAGE_BYTES)
        || plan.append.global_record().is_some()
        || plan.append.global_bitmap().is_some()
    {
        return Err(EditError::Preservation);
    }
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let source = source::capture(inputs, &mut reader)?;
    let census = map_authority::capture_catalog(
        image,
        reader.budget,
        cancelled,
        |schema, indexes, declarations, reader| {
            source.require_catalog_claims(schema, indexes, declarations, reader)
        },
    )?;
    census.require_exclusive(&source.owners, reader.budget, cancelled)?;
    // Bind the staged owner context to this source, but never use its Census
    // or prepared populations as independent source authority.
    plan.append.owner.census(image)?;
    let mut indexes = Vec::new();
    for physical in &source.physical {
        reader.tick()?;
        if plan
            .append
            .index_definition(physical.index, reader.budget, cancelled)?
            != physical.checked.schema().definition_page()
        {
            return Err(EditError::Preservation);
        }
        push(
            &mut indexes,
            maps::IndexOwner {
                definition: physical.checked.schema().definition_page(),
                index: physical.index,
            },
            &mut reader,
        )?;
    }
    let header = inspect_header(&mut Cursor::new(after), reader.budget, cancelled)?;
    let mut actual_source = Cursor::new(after);
    let mut actual = Reader {
        source: &mut actual_source,
        header,
        budget: reader.budget,
        cancelled,
        reads: 0,
    };
    let mut state = Replay {
        image,
        indexes: &indexes,
        owners: &source.owners,
        census: &census,
        plan,
        after,
        next_eof: image.catalog().header().page_count(),
        split_position: 0,
        edits: maps::qualified(),
        spans: Vec::new(),
    };
    for physical in source.physical {
        actual.tick()?;
        let checked = physical.checked;
        let updates = physical.updates;
        let index = physical.index;
        if !affects(checked, index, updates, &mut actual)? {
            continue;
        }
        for key in index.columns() {
            actual.work(checked.schema().columns().len())?;
            let column = checked
                .schema()
                .columns()
                .iter()
                .find(|column| column.number() == key.number())
                .ok_or(InspectError::Malformed)?;
            if column.scalar_type() != Some(ScalarType::Int32)
                || !column.fixed()
                || column.storage_length() != size_of::<i32>()
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        match index.columns().len() {
            1 => {
                let column = checked
                    .schema()
                    .columns()
                    .iter()
                    .position(|column| column.number() == index.columns()[0].number())
                    .ok_or(InspectError::Malformed)?;
                let profile = SingleProfile {
                    column,
                    kind: NumericKey::new(ScalarType::Int32)?,
                    descending: index.columns()[0].descending(),
                };
                state.index::<_, { encoding::MAX_ENTRY_BYTES }>(
                    checked,
                    updates,
                    index,
                    &profile,
                    &mut actual,
                )?;
            }
            2..=10 => {
                let profile = TupleProfile::<i64>::new_for_delete(
                    index,
                    checked.schema(),
                    actual.budget,
                    cancelled,
                )?;
                state.index::<_, { composite::MAX_ENTRY_BYTES }>(
                    checked,
                    updates,
                    index,
                    &profile,
                    &mut actual,
                )?;
            }
            _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        }
    }
    state.finish(&mut actual)
}

struct Replay<'a, O: OwnerContext> {
    image: &'a NativeImage,
    indexes: &'a [maps::IndexOwner<'a>],
    owners: &'a [map_authority::Owner],
    census: &'a map_authority::Census,
    plan: &'a Plan<O>,
    after: &'a [u8],
    next_eof: u32,
    split_position: usize,
    edits: maps::Expected,
    spans: Vec<Range<usize>>,
}

impl<O: OwnerContext> Replay<'_, O> {
    fn index<P: WriteProfile, const N: usize>(
        &mut self,
        checked: &TableCheck,
        updates: &[IndexKeyUpdate],
        index: &NativeIndex,
        profile: &P,
        actual: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let mut source = self.image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: self.image.catalog().header(),
            budget: actual.budget,
            cancelled: actual.cancelled,
            reads: 0,
        };
        let population = population::derive::<P, N>(
            self.image,
            checked.schema(),
            index,
            updates,
            profile,
            &mut reader,
        )?;
        let counter = history::prove(
            self.image,
            checked.schema(),
            index,
            population.history_delta,
            self.after,
            &mut reader,
        )?;
        push(&mut self.spans, counter, &mut reader)?;
        let mut pages = crate::format::allocation::pages(&mut reader, index.usage_map())?;
        for &number in &pages {
            let start = number as usize * PAGE_BYTES;
            push(&mut self.spans, start..start + PAGE_BYTES, &mut reader)?;
        }
        let column = checked
            .schema()
            .columns()
            .iter()
            .position(|column| column.number() == index.columns()[0].number())
            .ok_or(InspectError::Malformed)?;
        retained::prove_population::<P, N>(
            &retained::Operation {
                image: self.image,
                index,
                owner: checked.schema().definition_page(),
                column,
                profile,
                tree: &population.tree,
                keys: &population.keys,
            },
            self.plan,
            actual,
            &mut retained::Progress {
                next_eof: &mut self.next_eof,
                map_edits: &mut self.edits,
                pages: &mut pages,
                split_position: &mut self.split_position,
            },
        )?;
        let levels = pages.len().checked_ilog2().unwrap_or(0) as usize + 1;
        actual.work(pages.len().saturating_mul(levels))?;
        pages.sort_unstable();
        if crate::format::allocation::pages(actual, index.usage_map())? != pages {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    fn finish(
        mut self,
        actual: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Vec<Range<usize>>, EditError> {
        if self.next_eof as usize * PAGE_BYTES != self.after.len()
            || self.split_position != self.plan.certificates.len()
            || self.plan.added.len()
                != self
                    .next_eof
                    .checked_sub(self.image.catalog().header().page_count())
                    .ok_or(EditError::Preservation)? as usize
        {
            return Err(EditError::Preservation);
        }
        let mut source = self.image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: self.image.catalog().header(),
            budget: actual.budget,
            cancelled: actual.cancelled,
            reads: 0,
        };
        maps::authority_bound(
            self.census,
            self.owners,
            self.indexes,
            self.plan,
            &self.edits,
            &mut reader,
        )?;
        maps::finish(&mut reader, &self.edits, self.plan, self.after)?;
        maps::inventory(
            &mut reader,
            &self.edits,
            self.plan,
            self.indexes,
            self.next_eof,
        )?;
        for span in maps::spans(&self.edits, &mut reader)? {
            push(&mut self.spans, span, &mut reader)?;
        }
        for (position, page) in self.plan.added.iter().enumerate() {
            reader.tick()?;
            let number = self.image.catalog().header().page_count() as usize + position;
            if page.number as usize != number
                || page.bytes.len() != PAGE_BYTES
                || self
                    .after
                    .get(number * PAGE_BYTES..(number + 1) * PAGE_BYTES)
                    != Some(page.bytes.as_slice())
            {
                return Err(EditError::Preservation);
            }
        }
        for patch in &self.plan.patches {
            reader.tick()?;
            reader.work(self.spans.len())?;
            let end = patch
                .offset
                .checked_add(patch.after.len())
                .ok_or(EditError::Preservation)?;
            if patch.before.len() != patch.after.len()
                || self.image.as_bytes().get(patch.offset..end) != Some(patch.before.as_slice())
                || self.after.get(patch.offset..end) != Some(patch.after.as_slice())
                || !self
                    .spans
                    .iter()
                    .any(|span| span.start <= patch.offset && end <= span.end)
            {
                return Err(EditError::Preservation);
            }
        }
        Ok(self.spans)
    }
}

fn affects(
    checked: &TableCheck,
    index: &NativeIndex,
    updates: &[IndexKeyUpdate],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    reader.work(updates.len().saturating_mul(index.columns().len()))?;
    Ok(updates.iter().any(|update| {
        checked
            .schema()
            .columns()
            .get(update.column)
            .is_some_and(|column| {
                index
                    .columns()
                    .iter()
                    .any(|key| key.number() == column.number())
            })
    }))
}

fn push<T>(
    values: &mut Vec<T>,
    value: T,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader
        .budget
        .charge_allocation(1, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    values.push(value);
    Ok(())
}
