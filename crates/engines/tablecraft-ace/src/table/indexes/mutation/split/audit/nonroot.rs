//! Re-derive recursive splits from original counts, keys and page capacities.
mod framing;
mod replay;

use super::super::super::{retained::RetainedProfile, tree::Tree};
use super::maps;
use crate::{
    EditError, NativeImage,
    format::reader::Reader,
    table::indexes::page::{Entry, KeyRef},
};
use std::io::Cursor;

pub(super) struct Operation<'a, P: RetainedProfile, const N: usize> {
    pub image: &'a NativeImage,
    pub tree: &'a Tree,
    pub keys: &'a [KeyRef<P::Key>],
    pub profile: &'a P,
    pub owner: u32,
    pub usage: u32,
}

pub(super) struct Progress<'a> {
    pub next_eof: &'a mut u32,
    pub map_edits: &'a mut maps::Expected,
}

pub(super) fn prove<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P, N>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<Vec<u32>, EditError> {
    replay::prove(operation, actual, progress)
}

pub(super) fn fits<P: RetainedProfile, const N: usize>(
    entries: &[Entry<&P::Key>],
    seed: usize,
    profile: &P,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    framing::fits::<P, N>(entries, seed, profile, actual)
}

/// Shared audit-owned framing proof for both recursive and fast paths.
pub(super) fn layout<P: RetainedProfile, const N: usize>(
    entries: &[Entry<&P::Key>],
    seed: usize,
    raw: &[u8],
    profile: &P,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    framing::layout::<P, N>(entries, seed, raw, profile, actual)
}
