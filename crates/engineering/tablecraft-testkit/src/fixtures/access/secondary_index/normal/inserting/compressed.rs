//! A genuine preexisting compressed Guard survives CREATE, UPDATE and INSERT.
use crate::fixtures::access::{
    multiple_indexes,
    packed_record::append_record,
    put32, raw_forwarding_chain,
    secondary_index::{normal::updating, numeric},
};
use tablecraft_core::values::{Currency, Value};

const PAGE: usize = 4096;

pub struct Case {
    pub creation: numeric::Source,
    pub updated: Vec<u8>,
    pub expected: Vec<u8>,
    pub guard_root: u32,
    pub changes: Vec<updating::Cell>,
}

/// Supply a nonempty Unicode value under the actual seven-unit native width.
pub fn values() -> Vec<Value> {
    vec![
        Value::Int32(99),
        Value::Null,
        Value::Currency(Currency::from_scaled(12_345)),
        Value::Text("新🦀".into()),
    ]
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn physical(bytes: &[u8], definition: u32) -> usize {
    let base = definition as usize * PAGE;
    let mut at = base + 63 + long(bytes, base + 51) as usize * 12 + word(bytes, base + 45) * 25;
    for _ in 0..word(bytes, base + 45) {
        at += 2 + word(bytes, at);
    }
    at
}

fn wires(values: impl IntoIterator<Item = (Option<i32>, u32)>, descending: bool) -> Vec<Vec<u8>> {
    let mut entries = values
        .into_iter()
        .map(|(value, reference)| {
            let mut entry = numeric::key(numeric::Kind::Int32, value.map(i64::from), descending);
            entry.extend(reference.to_be_bytes());
            entry
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn frame(bytes: &mut [u8], root: u32, definition: u32, entries: &[Vec<u8>], prefix: usize) {
    let start = root as usize * PAGE;
    let expected = multiple_indexes::leaf_owned(
        Some(&bytes[start..start + PAGE]),
        entries,
        prefix,
        0,
        0,
        definition,
    );
    bytes[start..start + PAGE].copy_from_slice(&expected);
}

/// The compressed owner exists BEFORE the new ordinary CREATE. A fresh small
/// CREATE would choose zero prefix, so its output is never falsely relabeled.
pub fn source(descending: bool) -> Case {
    let mut creation = numeric::source(
        numeric::Kind::Int32,
        numeric::Population::NullableDuplicate,
        descending,
    );
    let definition = creation.target_definition;
    let guard_root = long(
        &creation.before,
        physical(&creation.before, definition) + 38,
    );
    for bytes in [&mut creation.before, &mut creation.expected] {
        let descriptor = physical(bytes, definition);
        bytes[descriptor + 6] = u8::from(!descending);
        for row in &creation.rows {
            let range = raw_forwarding_chain::terminal_range(
                bytes,
                row.reference >> 8,
                (row.reference & 255) as usize,
            );
            let record = &mut bytes[range];
            assert_eq!(&record[..2], &[4, 0]);
            let last = record.len() - 1;
            record[last] &= !2;
        }
        let entries = wires(
            creation.rows.iter().map(|row| (None, row.reference)),
            descending,
        );
        frame(bytes, guard_root, definition, &entries, 3);
        assert_eq!(word(bytes, guard_root as usize * PAGE + 24), 3);
    }
    let mut updated = creation.expected.to_vec();
    let changes = creation
        .rows
        .iter()
        .enumerate()
        .map(|(ordinal, _)| updating::Cell {
            row: ordinal,
            expected: None,
            value: Some(5),
        })
        .collect();
    for row in &creation.rows {
        let range = raw_forwarding_chain::terminal_range(
            &updated,
            row.reference >> 8,
            (row.reference & 255) as usize,
        );
        let record = &mut updated[range];
        record[6..10].copy_from_slice(&5_i32.to_le_bytes());
        let last = record.len() - 1;
        record[last] |= 2;
    }
    let entries = wires(
        creation.rows.iter().map(|row| (Some(5), row.reference)),
        descending,
    );
    frame(&mut updated, guard_root, definition, &entries, 3);
    assert_eq!(word(&updated, guard_root as usize * PAGE + 24), 3);

    let mut expected = updated.to_vec();
    let data = (definition as usize + 1) * PAGE;
    let slot = word(&updated, data + 12);
    assert_eq!(slot, creation.rows.len());
    let reference = ((definition + 1) << 8) | slot as u32;
    let mut record = vec![4, 0];
    record.extend(99_i32.to_le_bytes());
    record.extend(0_i32.to_le_bytes());
    record.extend(12_345_i64.to_le_bytes());
    record.extend("新🦀".encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(record.len(), 24);
    record.extend([24, 0, 18, 0, 1, 0, 13]);
    append_record(&mut expected[data..data + PAGE], &record);
    let base = definition as usize * PAGE;
    put32(&mut expected, base + 16, long(&updated, base + 16) + 1);
    for counter in [67, 79] {
        put32(
            &mut expected,
            base + counter,
            long(&updated, base + counter) + 1,
        );
    }
    let guard_entries = wires(
        creation
            .rows
            .iter()
            .map(|row| (Some(5), row.reference))
            .chain([(None, reference)]),
        descending,
    );
    frame(&mut expected, guard_root, definition, &guard_entries, 3);
    assert_eq!(word(&expected, guard_root as usize * PAGE + 24), 0);
    let key_entries = wires(
        creation
            .rows
            .iter()
            .map(|row| {
                (
                    row.value.map(|value| i32::try_from(value).unwrap()),
                    row.reference,
                )
            })
            .chain([(Some(99), reference)]),
        descending,
    );
    frame(&mut expected, creation.root, definition, &key_entries, 0);
    assert_eq!(
        &expected[base + 20..base + 24],
        &updated[base + 20..base + 24]
    );
    assert_eq!(expected.len(), updated.len());
    Case {
        creation,
        updated,
        expected,
        guard_root,
        changes,
    }
}
