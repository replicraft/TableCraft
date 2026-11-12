//! Shared original-key update, ordering and constraint proof before serialization.
mod requests;
use super::super::{IndexKeyUpdate, encoding::Mode, profile::WriteProfile, tree::Tree};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, format::reader::Reader,
    table::indexes::page::KeyRef,
};
use requests::Requests;
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
    prepare_with_population_limit::<_, N>(image, reader, index, owner, updates, profile, false)
}

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::indexes::mutation) fn prepare_retained_long<
    P: WriteProfile,
    const N: usize,
>(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    updates: &[IndexKeyUpdate],
    profile: &P,
) -> Result<Prepared<P::Key>, EditError> {
    prepare_with_population_limit::<_, N>(image, reader, index, owner, updates, profile, true)
}

fn prepare_with_population_limit<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    updates: &[IndexKeyUpdate],
    profile: &P,
    retained_long: bool,
) -> Result<Prepared<P::Key>, EditError> {
    let mut entries = Vec::new();
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    let tree = if retained_long {
        Tree::read_retained_long::<_, N>(
            image,
            reader,
            owner,
            index.root_page(),
            profile,
            0,
            &mut entries,
        )?
    } else {
        Tree::read::<_, N>(
            image,
            reader,
            owner,
            index.root_page(),
            profile,
            0,
            &mut entries,
        )?
    };
    #[cfg(not(all(feature = "journal", feature = "ace16-create")))]
    let tree = {
        let _ = retained_long;
        Tree::read::<_, N>(
            image,
            reader,
            owner,
            index.root_page(),
            profile,
            0,
            &mut entries,
        )?
    };
    let requests = Requests::new(updates, profile, reader.budget, reader.cancelled)?;
    let mut matched = 0_usize;
    for key in &mut entries {
        reader.tick()?;
        for &position in requests.row(key.row, reader.budget, reader.cancelled)? {
            reader.work(profile.components())?;
            let update = &updates[position];
            profile.update(&mut key.key, update.column, update.expected, update.value)?;
            matched = matched.checked_add(1).ok_or(InspectError::StructureLimit)?;
        }
    }
    if matched != requests.count() {
        return Err(EditError::Preservation);
    }
    let levels = entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let sorting = entries
        .len()
        .checked_mul(levels)
        .and_then(|work| work.checked_mul(size_of::<KeyRef<P::Key>>()))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(sorting)?;
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
        || requests.has_null_transition()
    {
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
