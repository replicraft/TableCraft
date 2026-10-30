//! Paid owned-key preparation shared by fitting replacement and split consumers.
use super::super::{
    retained::{Preserved, PreservedKey},
    row_key::RowKeyProfile,
    tree::Tree,
};
use super::rows::{Final, KeyChange, Original};
use crate::{
    EditError, NativeImage, NativeIndex,
    format::reader::Reader,
    table::indexes::{
        key::{KeyProfile, KeyWork},
        page::KeyRef,
    },
};
use std::{cmp::Ordering, io::Cursor};

pub(in crate::table::indexes::mutation) struct Prepared<P: RowKeyProfile> {
    pub tree: Tree,
    pub entries: Vec<KeyRef<PreservedKey<P::Key>>>,
    pub profile: Preserved<P>,
}

pub(in crate::table::indexes::mutation) fn prepare<P: RowKeyProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    owner: u32,
    index: &NativeIndex,
    original: P,
    updates: &[impl KeyChange],
) -> Result<Prepared<P>, EditError> {
    let profile = Preserved(original);
    let mut entries = Vec::new();
    let tree = Tree::read::<_, N>(
        image,
        reader,
        owner,
        index.root_page(),
        &profile,
        0,
        &mut entries,
    )?;

    let mut matched = 0;
    for entry in &mut entries {
        reader.tick()?;
        reader.work(updates.len())?;
        if let Some(update) = updates
            .iter()
            .find(|update| update.reference() == entry.row)
        {
            reader.work(2 * profile.components() * update.columns().len())?;
            let mut work = KeyWork {
                budget: reader.budget,
                cancelled: reader.cancelled,
            };
            let before = profile.row_key(&Original(update), &mut work)?;
            if profile.compare(&entry.key, &before) != Ordering::Equal {
                return Err(EditError::Conflict);
            }
            let wire = profile.0.encoded_row(&Final(update), &mut work)?;
            entry.key = profile.decode(&wire, &mut work)?.0;
            matched += 1;
        }
    }

    if matched != updates.len() {
        return Err(EditError::Preservation);
    }

    if updates.is_empty() {
        return Ok(Prepared {
            tree,
            entries,
            profile,
        });
    }

    let levels = entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(entries.len() * levels * profile.max_key_bytes())?;
    entries.sort_unstable_by(|left, right| left.compare_with(right, &profile));

    Ok(Prepared {
        tree,
        entries,
        profile,
    })
}
