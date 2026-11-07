//! Independent literal TCNA2 grammar for supplied/omitted test inputs.
use tablecraft_core::values::Value;

/// Inputs and expected lengths remain fixture-owned, never production-derived.
/// Writes legacy carrier tags independently of the production codec.
pub fn original_presence_wire(
    table: &str,
    before_length: usize,
    binding: [u8; 32],
    rows: Vec<Vec<Option<Value>>>,
) -> Vec<u8> {
    presence_wire(table, before_length, binding, rows, false)
}

/// Independent TCNA6 wider supplied carriers; omission uses its distinct tag14.
pub fn wider_presence_wire(
    table: &str,
    before_length: usize,
    binding: [u8; 32],
    rows: Vec<Vec<Option<Value>>>,
) -> Vec<u8> {
    presence_wire(table, before_length, binding, rows, true)
}

fn presence_wire(
    table: &str,
    before_length: usize,
    binding: [u8; 32],
    rows: Vec<Vec<Option<Value>>>,
    wider: bool,
) -> Vec<u8> {
    let count = rows.len() as u32;
    let mut body = Vec::new();
    for fields in rows {
        let start = body.len();
        body.extend_from_slice(&0_u32.to_le_bytes());
        body.extend_from_slice(&(fields.len() as u32).to_le_bytes());
        for field in fields {
            let (tag, bytes) = match field {
                None => (if wider { 14 } else { 8 }, Vec::new()),
                Some(Value::Null) => (0, Vec::new()),
                Some(Value::Boolean(value)) => (1, vec![u8::from(value)]),
                Some(Value::Int32(value)) => (2, value.to_le_bytes().to_vec()),
                Some(Value::Int64(value)) => (3, value.to_le_bytes().to_vec()),
                Some(Value::Currency(value)) => (4, value.scaled().to_le_bytes().to_vec()),
                Some(Value::DateTime(value)) => (5, value.to_bits().to_le_bytes().to_vec()),
                Some(Value::Text(value)) => (6, value.into_bytes()),
                Some(Value::Binary(value)) => (7, value),
                Some(Value::Byte(value)) if wider => (8, vec![value]),
                Some(Value::Int16(value)) if wider => (9, value.to_le_bytes().to_vec()),
                Some(Value::Single(value)) if wider => (10, value.to_bits().to_le_bytes().to_vec()),
                Some(Value::Double(value)) if wider => (11, value.to_bits().to_le_bytes().to_vec()),
                Some(Value::Guid(value)) if wider => (12, value.to_bytes().to_vec()),
                Some(Value::Decimal(value)) if wider => {
                    let mut bytes = value.coefficient().to_le_bytes().to_vec();
                    bytes.push(value.scale());
                    (13, bytes)
                }
                _ => panic!("closed original literal presence carriers"),
            };
            body.extend_from_slice(&[tag, 0, 0, 0]);
            body.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            body.extend_from_slice(&bytes);
        }
        let length = (body.len() - start) as u32;
        body[start..start + 4].copy_from_slice(&length.to_le_bytes());
    }
    let mut raw = Vec::from(if wider { *b"TCNA0006" } else { *b"TCNA0002" });
    raw.extend_from_slice(&[0; 4]);
    raw.extend_from_slice(&[if wider { 6 } else { 2 }, 0, 0, 0]);
    raw.extend_from_slice(&(before_length as u64).to_le_bytes());
    raw.extend_from_slice(&binding);
    for number in [table.len() as u32, count, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(table.as_bytes());
    raw.extend_from_slice(&body);
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_le_bytes());
    raw
}
