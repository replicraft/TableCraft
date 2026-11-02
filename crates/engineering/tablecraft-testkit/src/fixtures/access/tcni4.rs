//! Literal original single-column I32/NULL framing, independent of engine codecs.
use tablecraft_core::values::Value;

pub fn single_column_original_wire(
    table: &str,
    column: &str,
    updates: &[(usize, Value, Value)],
    before_length: usize,
    binding: [u8; 32],
) -> Vec<u8> {
    let total = 72 + table.len() + 12 + column.len() + 26 * updates.len();
    let mut wire = Vec::from(*b"TCNI0004");
    wire.extend_from_slice(&(total as u32).to_le_bytes());
    wire.extend_from_slice(&[4, 0, 0, 0]);
    wire.extend_from_slice(&(before_length as u64).to_le_bytes());
    wire.extend_from_slice(&binding);
    for number in [table.len() as u32, 1, updates.len() as u32, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(table.as_bytes());
    for number in [column.len() as u32, updates.len() as u32, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(column.as_bytes());
    for (row, expected, value) in updates {
        wire.extend_from_slice(&(*row as u64).to_le_bytes());
        for value in [expected, value] {
            match value {
                Value::Int32(value) => {
                    wire.push(2);
                    wire.extend_from_slice(&u64::from(*value as u32).to_le_bytes());
                }
                Value::Null => wire.extend_from_slice(&[0; 9]),
                _ => panic!("outside literal I32/NULL carrier law"),
            }
        }
    }
    assert_eq!(wire.len(), total);
    wire
}
