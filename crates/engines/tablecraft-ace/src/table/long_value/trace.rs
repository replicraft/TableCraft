//! Optional paid reference tracing through the actual long-value decoder.
use super::{
    CHAINED, INLINE, LENGTH_TYPE_MASK, SINGLE_ROW, definition::Definition, read_with_trace,
};
use crate::{InspectError, NativeReadSource, format::reader::Reader};

#[derive(Clone, Copy)]
pub(in crate::table) struct Chunk {
    pub reference: u32,
    pub offset: usize,
    pub bytes: usize,
}

#[derive(Clone, Copy)]
pub(in crate::table) enum Kind {
    Null,
    Inline,
    Single,
    Chained,
}

pub(in crate::table) struct Traced {
    pub value: Option<Vec<u8>>,
    pub kind: Kind,
    pub chunks: Vec<Chunk>,
}

pub(in crate::table) fn read<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    descriptor: &[u8],
) -> Result<Traced, InspectError> {
    let mut chunks = Vec::new();
    let value = read_with_trace(reader, descriptor, Some(&mut chunks))?;
    let kind = kind(descriptor)?;
    Ok(Traced {
        value,
        kind,
        chunks,
    })
}

/// Classify only the native carrier header, without following its references.
/// Callers retain their own framing, budgets and semantic/source admission.
pub(in crate::table) fn kind(descriptor: &[u8]) -> Result<Kind, InspectError> {
    if descriptor.is_empty() {
        return Ok(Kind::Null);
    }
    match Definition::read(descriptor)?.tagged_length.get() & LENGTH_TYPE_MASK {
        INLINE => Ok(Kind::Inline),
        SINGLE_ROW => Ok(Kind::Single),
        CHAINED => Ok(Kind::Chained),
        _ => Err(InspectError::Unsupported),
    }
}

/// Match the exact canonical single-row producer descriptor without following it.
/// Generic native reads continue to preserve uninterpreted reserved bytes.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table) fn canonical_single(
    bytes: &[u8],
    reference: u32,
    length: usize,
) -> Result<bool, InspectError> {
    if bytes.len() != size_of::<Definition>()
        || length == 0
        || length > (!LENGTH_TYPE_MASK) as usize
    {
        return Ok(false);
    }
    let definition = Definition::read(bytes)?;
    Ok(
        definition.tagged_length.get() == (SINGLE_ROW | length as u32)
            && definition.reference.get() == reference
            && definition.reserved_zero(),
    )
}
