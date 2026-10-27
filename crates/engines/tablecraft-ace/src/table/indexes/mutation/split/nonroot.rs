//! Recursive physical planning; field authority stays in the table finalizer.
mod arena;
mod write;

use super::super::{IndexDataPatch, profile::SingleProfile, tree::Tree};
use super::Plan;
use crate::table::indexes::page::KeyRef;
use crate::{EditError, NativeImage, NativeIndex, format::reader::Reader};
use std::io::Cursor;

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub index: &'a NativeIndex,
    pub profile: &'a SingleProfile,
    pub keys: &'a [KeyRef],
}

pub(super) fn edit(
    operation: &Operation<'_>,
    tree: &Tree,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    let mut arena = arena::prepare(operation, tree, plan, reader)?;
    write::emit(operation, &mut arena, plan, patches, reader)
}
