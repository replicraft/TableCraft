//! Keep typed profiles and certified final populations through serialization.
use super::super::source;
use super::reserve;
use crate::table::mutation::append::owners::OwnerContext;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::{data_type::INT32, reader::Reader},
    table::indexes::{
        composite,
        mutation::{
            IndexKeyUpdate, NumericKey,
            composite::TupleProfile,
            encoding, population,
            profile::{SingleProfile, WriteProfile},
            split::{Plan, edit_prepared, nonroot},
        },
        profile::Coverage,
    },
};
use std::io::Cursor;
use tablecraft_schema::ScalarType;

pub(super) struct Typed<'a, P: WriteProfile> {
    index: &'a NativeIndex,
    profile: P,
    population: population::Prepared<P::Key>,
    first_column: usize,
}

#[allow(
    clippy::large_enum_variant,
    reason = "Inline profiles occupy a fallibly reserved, budget-metered owner vector; boxing would add a separate allocation for each owner"
)]
pub(super) enum Prepared<'a> {
    Single(Typed<'a, SingleProfile>),
    Tuple(Typed<'a, TupleProfile<i64>>),
}

pub(super) fn prepare<'a>(
    image: &NativeImage,
    schema: &TableSchema,
    owner: &source::Physical<'a>,
    updates: &[IndexKeyUpdate],
    complete: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Option<Prepared<'a>>, EditError> {
    let index = owner.index;
    reader.work(
        index
            .columns()
            .len()
            .saturating_mul(updates.len())
            .saturating_mul(schema.columns().len()),
    )?;
    if !complete
        && !index.columns().iter().any(|key| {
            updates.iter().any(|update| {
                schema
                    .columns
                    .get(update.column)
                    .is_some_and(|column| column.number() == key.number())
            })
        })
    {
        return Ok(None);
    }
    let mut columns = Vec::new();
    reserve(&mut columns, index.columns().len(), reader)?;
    for key in index.columns() {
        reader.tick()?;
        reader.work(schema.columns().len())?;
        let (position, column) = schema
            .columns()
            .iter()
            .enumerate()
            .find(|(_, column)| column.number() == key.number())
            .ok_or(InspectError::Malformed)?;
        if column.native_type() != INT32
            || !column.fixed()
            || column.storage_length() != size_of::<i32>()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        columns.push(position);
    }
    let first_column = *columns.first().ok_or(InspectError::Malformed)?;
    if columns.len() == 1 {
        let profile = SingleProfile {
            column: first_column,
            kind: NumericKey::new(ScalarType::Int32)?,
            descending: index.columns()[0].descending(),
        };
        let population = population::prepare::<_, { encoding::MAX_ENTRY_BYTES }>(
            image, reader, index, schema, updates, &profile,
        )?;
        source::require_tree(&population.tree, &owner.pages, reader)?;
        population::history::preflight(image, schema, index, population.increment, reader)?;
        if !population.tree.is_leaf()
            && population.entries.len()
                < population
                    .tree
                    .leaf_count(reader.budget, reader.cancelled)?
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        Ok(Some(Prepared::Single(Typed {
            index,
            profile,
            population,
            first_column,
        })))
    } else {
        let profile = TupleProfile::<i64>::with_coverage(
            index,
            schema,
            Coverage::PhysicalMembership,
            reader.budget,
            reader.cancelled,
        )?;
        let population = population::prepare::<_, { composite::MAX_ENTRY_BYTES }>(
            image, reader, index, schema, updates, &profile,
        )?;
        source::require_tree(&population.tree, &owner.pages, reader)?;
        population::history::preflight(image, schema, index, population.increment, reader)?;
        if !population.tree.is_leaf()
            && population.entries.len()
                < population
                    .tree
                    .leaf_count(reader.budget, reader.cancelled)?
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        Ok(Some(Prepared::Tuple(Typed {
            index,
            profile,
            population,
            first_column,
        })))
    }
}

impl Prepared<'_> {
    pub(super) fn write(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        plan: &mut Plan<impl OwnerContext>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        match self {
            Self::Single(typed) => {
                typed.write::<{ encoding::MAX_ENTRY_BYTES }>(image, schema, plan, reader)
            }
            Self::Tuple(typed) => {
                typed.write::<{ composite::MAX_ENTRY_BYTES }>(image, schema, plan, reader)
            }
        }
    }
}

impl<P: WriteProfile> Typed<'_, P> {
    fn write<const N: usize>(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        plan: &mut Plan<impl OwnerContext>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if !self.population.changed {
            return Ok(());
        }
        let operation = nonroot::Operation::<P, N> {
            image,
            index: self.index,
            profile: &self.profile,
            keys: &self.population.entries,
        };
        let mut patches = std::mem::take(&mut plan.patches);
        let result = if self.population.tree.is_leaf() {
            edit_prepared(
                &operation,
                &self.population.tree,
                self.first_column,
                &mut patches,
                plan,
                reader,
            )
        } else {
            nonroot::edit_population(
                &operation,
                &self.population.tree,
                plan,
                reader,
                &mut patches,
            )
        };
        plan.patches = patches;
        result?;
        population::history::patch(
            image,
            schema,
            self.index,
            self.population.increment,
            &mut plan.patches,
            reader,
        )
    }
}
