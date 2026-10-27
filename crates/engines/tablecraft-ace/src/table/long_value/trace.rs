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
    let kind = if descriptor.is_empty() {
        Kind::Null
    } else {
        match Definition::read(descriptor)?.tagged_length.get() & LENGTH_TYPE_MASK {
            INLINE => Kind::Inline,
            SINGLE_ROW => Kind::Single,
            CHAINED => Kind::Chained,
            _ => return Err(InspectError::Unsupported),
        }
    };
    Ok(Traced {
        value,
        kind,
        chunks,
    })
}
