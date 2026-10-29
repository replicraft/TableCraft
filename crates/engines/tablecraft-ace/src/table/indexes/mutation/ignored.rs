//! Fitting IgnoreNULL rewrites consume complete original-row associations.
use super::{
    IndexKeyUpdate, PatchSet, population, profile::WriteProfile, tree::population::Population,
};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::reader::Reader,
};
use std::io::Cursor;

pub(super) fn edit<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    schema: &TableSchema,
    updates: &[IndexKeyUpdate],
    profile: &P,
    patches: &mut PatchSet,
) -> Result<(), EditError> {
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    if !constraints.ignore_nulls() || constraints.primary_key() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let prepared = population::prepare::<_, N>(image, reader, index, schema, updates, profile)?;
    if !prepared.changed {
        return Ok(());
    }
    let leaves = prepared.tree.leaf_count(reader.budget, reader.cancelled)?;
    if !prepared.tree.is_leaf() && prepared.entries.len() < leaves {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut population = Population {
        keys: &prepared.entries,
        leaves,
    };
    prepared.tree.rewrite_population::<_, N>(
        image,
        &mut population,
        profile,
        &mut patches.indexes,
        reader.budget,
        reader.cancelled,
    )?;
    if !population.keys.is_empty() || population.leaves != 0 {
        return Err(EditError::Preservation);
    }
    population::history::patch(
        image,
        schema,
        index,
        prepared.increment,
        &mut patches.indexes,
        reader,
    )
}
