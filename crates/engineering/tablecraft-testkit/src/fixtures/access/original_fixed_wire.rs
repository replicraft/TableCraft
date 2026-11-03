//! Literal original fixed UPDATE framing shared by actual TCNI4/8 consumers.
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
pub enum Dialect {
    Ordinary,
    Ieee,
}

pub struct Group<'a> {
    pub column: &'a str,
    pub updates: &'a [(usize, Value, Value)],
}

/// Existing single-column numeric oracle; all old accepted carriers stay exact.
pub fn single_column_original_wire(
    table: &str,
    column: &str,
    updates: &[(usize, Value, Value)],
    before_length: usize,
    binding: [u8; 32],
) -> Vec<u8> {
    for (_, before, after) in updates {
        for value in [before, after] {
            assert!(matches!(
                value,
                Value::Null | Value::Int32(_) | Value::Int64(_) | Value::Currency(_)
            ));
        }
    }
    original_wire(
        Dialect::Ordinary,
        table,
        &[Group { column, updates }],
        before_length,
        binding,
    )
}

/// Independent literal group envelope; no production codec/constants are used.
pub fn original_wire(
    dialect: Dialect,
    table: &str,
    groups: &[Group<'_>],
    before_length: usize,
    binding: [u8; 32],
) -> Vec<u8> {
    let assignments = groups
        .iter()
        .map(|group| group.updates.len())
        .sum::<usize>();
    let total = 72
        + table.len()
        + groups
            .iter()
            .map(|group| 12 + group.column.len() + 26 * group.updates.len())
            .sum::<usize>();
    let (magic, profile) = match dialect {
        Dialect::Ordinary => (b"TCNI0004", 4),
        Dialect::Ieee => (b"TCNI0008", 8),
    };
    let mut wire = Vec::from(*magic);
    wire.extend_from_slice(&(total as u32).to_le_bytes());
    wire.extend_from_slice(&[profile, 0, 0, 0]);
    wire.extend_from_slice(&(before_length as u64).to_le_bytes());
    wire.extend_from_slice(&binding);
    for number in [
        table.len() as u32,
        groups.len() as u32,
        assignments as u32,
        0,
    ] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(table.as_bytes());
    for group in groups {
        for number in [group.column.len() as u32, group.updates.len() as u32, 0] {
            wire.extend_from_slice(&number.to_le_bytes());
        }
        wire.extend_from_slice(group.column.as_bytes());
        for (row, before, after) in group.updates {
            wire.extend_from_slice(&(*row as u64).to_le_bytes());
            for value in [before, after] {
                let (tag, payload) = match value {
                    Value::Null => (0, [0; 8]),
                    Value::Boolean(value) => (1, u64::from(*value).to_le_bytes()),
                    Value::Int32(value) => (2, u64::from(*value as u32).to_le_bytes()),
                    Value::Int64(value) => (3, value.to_le_bytes()),
                    Value::Currency(value) => (4, value.scaled().to_le_bytes()),
                    Value::DateTime(value) => (5, value.to_bits().to_le_bytes()),
                    Value::Single(value) if matches!(dialect, Dialect::Ieee) => {
                        (6, u64::from(value.to_bits()).to_le_bytes())
                    }
                    Value::Double(value) if matches!(dialect, Dialect::Ieee) => {
                        (7, value.to_bits().to_le_bytes())
                    }
                    _ => panic!("outside literal fixed dialect carrier law"),
                };
                wire.push(tag);
                wire.extend_from_slice(&payload);
            }
        }
    }
    assert_eq!(wire.len(), total);
    wire
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::Single;

    fn literal(hex: &str) -> Vec<u8> {
        hex.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn original_single_column_numeric_wire_keeps_complete_literal_bytes() {
        let updates = [(2, Value::Int32(-1), Value::Null)];
        assert_eq!(
            single_column_original_wire("T", "X", &updates, 4096, [0x5a; 32]),
            literal(
                "54434e4930303034700000000400000000100000000000005a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a010000000100000001000000000000005401000000010000000000000058020000000000000002ffffffff00000000000000000000000000"
            )
        );
    }

    #[test]
    fn ieee_shared_frame_keeps_zero_padding_and_raw_negative_zero_bits() {
        let updates = [(
            2,
            Value::Single(Single::from_bits(0x8000_0000)),
            Value::Single(Single::from_bits(1)),
        )];
        assert_eq!(
            original_wire(
                Dialect::Ieee,
                "T",
                &[Group {
                    column: "S",
                    updates: &updates
                }],
                4096,
                [0x5a; 32]
            ),
            literal(
                "54434e4930303038700000000800000000100000000000005a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a0100000001000000010000000000000054010000000100000000000000530200000000000000060000008000000000060100000000000000"
            )
        );
    }
}
