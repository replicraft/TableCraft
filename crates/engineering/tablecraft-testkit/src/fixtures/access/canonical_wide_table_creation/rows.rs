//! Literal first-row defaults and the final field's distinct supplied presence.
use super::{FIELD_COUNT, PAGE, Source};
use crate::fixtures::access::generated_record_insert::placement::{self, Pages};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug)]
pub enum LastFieldInput {
    Omitted,
    Value(i32),
    Null,
}

/// A generated ID1 and 254 independent literal default-zero outcomes.
pub fn values(input: LastFieldInput) -> Vec<Value> {
    let mut values = vec![Value::Int32(0); FIELD_COUNT];
    values[0] = Value::Int32(1);
    values[254] = match input {
        LastFieldInput::Omitted => Value::Int32(0),
        LastFieldInput::Value(value) => Value::Int32(value),
        LastFieldInput::Null => Value::Null,
    };
    values
}

/// Build only the first inserted row: arbitrary sequential placement/candidate
/// changes are not inferred from this one-row oracle.
pub fn after(before: &[u8], source: &Source, input: LastFieldInput) -> Vec<u8> {
    let definition = source.definition_pages[0] as usize;
    let pages = Pages {
        definition,
        data: source.data_page as usize,
        index: source.index_page as usize,
    };
    assert_eq!(
        &before[definition * PAGE + 16..definition * PAGE + 24],
        &[0; 8]
    );
    assert_eq!(
        &before[pages.data * PAGE + 12..pages.data * PAGE + 14],
        &[0; 2]
    );
    assert_eq!(placement::next_id(before, pages), 1);
    let mut record = vec![255, 0];
    record.extend(1_i32.to_le_bytes());
    for _ in 1..255 {
        record.extend(0_i32.to_le_bytes());
    }
    if let LastFieldInput::Value(value) = input {
        record[1018..1022].copy_from_slice(&value.to_le_bytes());
    }
    record.extend([255; 31]);
    record.push(if matches!(input, LastFieldInput::Null) {
        63
    } else {
        127
    });
    assert_eq!(record.len(), 1054);
    placement::append(before, pages, 1, &record)
}
