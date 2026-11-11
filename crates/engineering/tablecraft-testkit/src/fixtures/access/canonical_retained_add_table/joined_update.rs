//! Independent two-table joined UPDATE input with a retained target and sibling.
use super::{
    Target, append, fields,
    rows::{self, Input, Profile},
};

pub struct Source {
    pub bytes: Vec<u8>,
    pub target_definition: u32,
}

/// Table1 has original ID/Score rows [1,17] and [2,NULL]. Changes adds independent
/// [1,5] and [2,7] records at actual EOF; the existing Other table stays untouched.
pub fn source() -> Source {
    let source = fields::source(fields::Case::FirstNumber);
    let definition = (source.bytes.len() / 4096) as u32;
    let empty = append(&source.bytes, "Changes", Target::GeneratedNumber);
    let first = rows::append(&empty, definition, Profile::NativeNumber, Input::Number(5));
    let bytes = rows::append(&first, definition, Profile::NativeNumber, Input::Number(7));
    Source {
        bytes,
        target_definition: source.definition,
    }
}
