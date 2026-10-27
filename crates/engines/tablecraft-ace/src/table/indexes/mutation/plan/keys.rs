//! Shared original-key update, ordering and constraint proof before serialization.
use super::super::{IndexKeyUpdate, encoding::Mode, profile::WriteProfile, tree::Tree};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, format::reader::Reader,
    table::indexes::page::KeyRef,
};
use std::io::Cursor;

pub(in crate::table::indexes::mutation) struct Prepared<K> {
    pub tree: Tree,
    pub entries: Vec<KeyRef<K>>,
    pub mode: Mode,
}

pub(in crate::table::indexes::mutation) fn prepare<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    updates: &[IndexKeyUpdate],
    profile: &P,
) -> Result<Prepared<P::Key>, EditError> {
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
    entries.sort_unstable_by(|a, b| a.compare_with(b, profile));
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let mut previous = None;
    for entry in &entries {
        reader.tick()?;
        let (all_null, any_null) = profile.constraint_nulls(&entry.key)?;
        constraints
            .check_tuple(
                &mut previous,
                if all_null { None } else { Some(entry.key) },
                any_null,
            )
            .map_err(InspectError::from)?;
    }
    let mode = if profile.requires_repack()
        || profile.components() > 1
        || updates.iter().any(|update| {
            profile.includes(update.column) && (update.expected.is_none() || update.value.is_none())
        }) {
        Mode::Repack
    } else {
        Mode::Fixed
    };
    Ok(Prepared {
        tree,
        entries,
        mode,
    })
}
