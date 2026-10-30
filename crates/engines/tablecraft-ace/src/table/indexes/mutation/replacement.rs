//! Genuine owned-key replacements for variable-slot UPDATE previews.
pub(super) mod keys;
pub(in crate::table::indexes::mutation) mod mixed;
mod qualification;
pub(in crate::table::indexes::mutation) mod rows;
use super::super::{
    IndexCheck, NativeIndex, binary, encoded_tuple, key::KeyRow, profile::Coverage, text,
};
use super::{IndexDataPatch, row_key::RowKeyProfile};
use crate::{
    EditError, NativeImage, TableSchema, WriteBlocker,
    format::{
        data_type::{SHORT_BINARY, SHORT_TEXT},
        reader::Reader,
    },
};
pub(in crate::table) use mixed::{patches as row_patches, qualify as qualify_row_indexes};
pub(in crate::table) use qualification::qualify;
use rows::KeyChange;
pub(in crate::table) use rows::RowKeyUpdate;
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(in crate::table) struct SlotKeyUpdate<'a> {
    pub reference: u32,
    pub columns: &'a [usize],
    pub before: &'a [Value],
    pub column: usize,
    pub value: &'a Value,
}

pub(in crate::table) struct SlotKeyChanges<'a> {
    pub column: usize,
    pub updates: &'a [SlotKeyUpdate<'a>],
}

impl KeyRow for SlotKeyUpdate<'_> {
    fn get(&self, column: usize) -> Option<&Value> {
        if column == self.column {
            Some(self.value)
        } else {
            self.columns
                .iter()
                .position(|position| *position == column)
                .and_then(|position| self.before.get(position))
        }
    }
}

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    column: usize,
    updates: &[SlotKeyUpdate<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    qualify(schema, indexes, column, budget, cancelled)?;

    let mut patches = Vec::new();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for (position, checked) in indexes.iter().enumerate() {
        reader.tick()?;
        let index = checked.index();
        if !index
            .columns()
            .iter()
            .any(|key| key.number() == schema.columns[column].number())
        {
            continue;
        }
        reader.work(indexes.len())?;
        if indexes[..position]
            .iter()
            .any(|previous| previous.index().data_number() == index.data_number())
        {
            continue;
        }

        if index.columns().len() == 1 {
            let metadata = &schema.columns[column];
            match metadata.native_type() {
                SHORT_TEXT => edit::<_, { text::MAX_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    schema.definition,
                    index,
                    text::TextProfile::new(column, metadata, index.columns()[0].descending())?,
                    updates,
                    &mut patches,
                )?,
                SHORT_BINARY => edit::<_, { binary::MAX_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    schema.definition,
                    index,
                    binary::BinaryProfile {
                        column,
                        descending: index.columns()[0].descending(),
                        maximum: metadata.storage_length(),
                        selection: binary::BinaryIndexSelection::All,
                    },
                    updates,
                    &mut patches,
                )?,
                _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
            }
        } else {
            let profile =
                encoded_tuple::EncodedTupleProfile::new(index, schema, Coverage::CompleteTable)?;
            edit::<_, { encoded_tuple::MAX_ENTRY_BYTES }>(
                image,
                &mut reader,
                schema.definition,
                index,
                profile,
                updates,
                &mut patches,
            )?;
        }
    }
    Ok(patches)
}

fn edit<P: RowKeyProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    owner: u32,
    index: &NativeIndex,
    original: P,
    updates: &[impl KeyChange],
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    let keys::Prepared {
        tree,
        entries,
        profile,
    } = keys::prepare::<P, N>(image, reader, owner, index, original, updates)?;
    if updates.is_empty() {
        return Ok(());
    }

    let mut remaining = entries.as_slice();
    tree.replace::<_, N>(
        image,
        &mut remaining,
        &profile,
        patches,
        reader.budget,
        reader.cancelled,
    )?;
    if !remaining.is_empty() {
        return Err(EditError::Preservation);
    }
    // Full after-image checks validate actual row NULL facts, uniqueness,
    // required fields and every reconstructed key before publication.
    Ok(())
}
