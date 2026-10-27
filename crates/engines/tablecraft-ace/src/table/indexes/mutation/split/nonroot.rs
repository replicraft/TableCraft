//! Recursive physical planning; field authority stays in the table finalizer.
mod arena;
mod write;

use super::super::{IndexDataPatch, retained::RetainedProfile, tree::Tree};
use super::Plan;
use crate::table::indexes::page::KeyRef;
use crate::{EditError, NativeImage, NativeIndex, format::reader::Reader};
use std::io::Cursor;

pub(super) struct Operation<'a, P: RetainedProfile, const N: usize> {
    pub image: &'a NativeImage,
    pub index: &'a NativeIndex,
    pub profile: &'a P,
    pub keys: &'a [KeyRef<P::Key>],
}

pub(super) fn edit<P: RetainedProfile, const N: usize>(
    operation: &Operation<'_, P, N>,
    tree: &Tree,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    let mut arena = arena::prepare(operation, tree, plan, reader)?;
    write::emit(operation, &mut arena, plan, patches, reader)
}
