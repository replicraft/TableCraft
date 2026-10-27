//! Re-derive recursive splits from original counts, keys and page capacities.
mod framing;
mod replay;

use super::super::super::{profile::SingleProfile, tree::Tree};
use super::maps;
use crate::{
    EditError, NativeImage,
    format::reader::Reader,
    table::indexes::page::{Entry, KeyRef},
};
use std::io::Cursor;

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub tree: &'a Tree,
    pub keys: &'a [KeyRef],
    pub profile: &'a SingleProfile,
    pub owner: u32,
    pub usage: u32,
}

pub(super) struct Progress<'a> {
    pub next_eof: &'a mut u32,
    pub map_edits: &'a mut Vec<maps::Expected>,
}

pub(super) fn prove(
    operation: &Operation<'_>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<Vec<u32>, EditError> {
    replay::prove(operation, actual, progress)
}

pub(super) fn fits(
    entries: &[Entry],
    seed: usize,
    profile: &SingleProfile,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    framing::fits(entries, seed, profile, actual)
}
