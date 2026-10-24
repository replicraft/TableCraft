//! Stored Memo calculations wrap text inside native long-value storage.
use super::super::{long_text, long_value};
use super::wrapper;
use crate::{InspectError, NativeReadSource, format::reader::Reader};
use tablecraft_core::values::Value;

pub(in crate::table) fn read<R: NativeReadSource>(
    definition: &[u8],
    reader: &mut Reader<'_, R>,
) -> Result<Value, InspectError> {
    let Some(wrapped) = long_value::read(reader, definition)? else {
        return Ok(Value::Null);
    };
    let bytes = wrapper::payload(&wrapped)?;
    Ok(Value::Text(long_text::decode(reader, bytes)?))
}
