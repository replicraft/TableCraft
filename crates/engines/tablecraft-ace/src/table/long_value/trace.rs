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
