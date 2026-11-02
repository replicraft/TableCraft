//! Independent TCNA wire witnesses; source loader is shared with direct proof tests.
use super::budget;
use sha2::{Digest, Sha256};
use tablecraft_ace::{NativeInsertIntent, RowInsert};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_desc_nullable_insert as sources;

pub(super) fn cases() -> Vec<sources::Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

pub(super) fn requests(count: usize) -> Vec<RowInsert> {
    assert!(count <= 2);
    (0..count)
        .map(|position| RowInsert::new(sources::values(position == 1)))
        .collect()
}

pub(super) fn wire(before: &[u8], rows: &[Vec<Value>]) -> Vec<u8> {
    let mut body = Vec::new();
    for row in rows {
        assert_eq!(row.len(), 3);
        let length: u32 = if matches!(row[2], Value::Null) {
            40
        } else {
            44
        };
        body.extend_from_slice(&length.to_le_bytes());
        body.extend_from_slice(&3_u32.to_le_bytes());
        for value in row {
            match value {
                Value::Int32(value) => {
                    body.extend_from_slice(&[2, 0, 0, 0]);
                    body.extend_from_slice(&4_u32.to_le_bytes());
                    body.extend_from_slice(&value.to_le_bytes());
                }
                Value::Null => {
                    body.extend_from_slice(&[0; 8]);
                }
                _ => panic!("outside frozen source request carriers"),
            }
        }
    }
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&((73 + body.len()) as u32).to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    raw.extend_from_slice(&digest.finalize());
    for number in [5_u32, rows.len() as u32, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    raw.extend_from_slice(&body);
    assert_eq!(raw.len(), 73 + body.len());
    raw
}

pub(super) fn intent(case: &sources::Case, count: usize) -> NativeInsertIntent {
    let rows = (0..count)
        .map(|position| sources::values(position == 1))
        .collect::<Vec<_>>();
    NativeInsertIntent::decode(&wire(&case.before, &rows), &mut budget(), &|| false).unwrap()
}
