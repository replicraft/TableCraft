//! Frozen independent multirow images and literal TCNA request bytes.
use super::budget;
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};
use tablecraft_ace::{NativeInsertIntent, RowInsert};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert as first;

pub(super) struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
    pub reverse: bool,
}

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

pub(super) fn cases() -> Vec<Case> {
    // Reuse the existing original loader; verify its entire first source set
    // before consuming any new multirow image or exercising the native writer.
    let originals = first::load_all(verify);
    let directory =
        PathBuf::from(env::var_os("TABLECRAFT_NORMAL_MULTIROW_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, reverse, after_sha) in [
        (
            5,
            false,
            "db9defca73f6013038d2608b9b8e62dd8355d1483039d3a46edad91612fa99ed",
        ),
        (
            5,
            true,
            "fa9eae3cbe20c9742c4f165ee84f510ff8e25b0437a76828bd95cd02b300d908",
        ),
        (
            6,
            false,
            "48b1070f78f68007c50f1409d3b013cdb2140495b1117035be6b847eb3f3d150",
        ),
        (
            6,
            true,
            "a9a64d255718d89ea4c9ada2ce020984ee0742305a7f821c4dd805d19d4b3f32",
        ),
    ] {
        let original = originals.iter().find(|case| case.header == header).unwrap();
        let order = if reverse { "tail-new" } else { "new-tail" };
        let prefix = format!("h{header}-multirow-{order}");
        let before = fs::read(directory.join(format!("{prefix}-before.accdb"))).unwrap();
        let after = fs::read(directory.join(format!("{prefix}-inserted.accdb"))).unwrap();
        assert_eq!(before, original.before);
        assert_eq!(before.len(), after.len());
        verify(&after, after_sha);
        cases.push(Case {
            before,
            after,
            definition: original.definition,
            data: original.data,
            reverse,
        });
    }
    assert_eq!(cases.len(), 4);
    cases
}

pub(super) fn rows(reverse: bool) -> Vec<Vec<Value>> {
    let new = first::values();
    let tail = vec![Value::Int32(25), Value::Text("tail".into()), Value::Null];
    if reverse {
        vec![tail, new]
    } else {
        vec![new, tail]
    }
}

pub(super) fn requests(reverse: bool) -> Vec<RowInsert> {
    rows(reverse).into_iter().map(RowInsert::new).collect()
}

pub(super) fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&155_u32.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    raw.extend_from_slice(&digest.finalize());
    for number in [8_u32, 2, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(b"Sentinel");
    let values: [(u32, &[u8], &[u8]); 2] = if reverse {
        [(40, &[25, 0, 0, 0], b"tail"), (39, &[24, 0, 0, 0], b"new")]
    } else {
        [(39, &[24, 0, 0, 0], b"new"), (40, &[25, 0, 0, 0], b"tail")]
    };
    for (length, id, text) in values {
        raw.extend_from_slice(&length.to_le_bytes());
        raw.extend_from_slice(&3_u32.to_le_bytes());
        for (tag, payload) in [(2, id), (6, text), (0, &[][..])] {
            raw.extend_from_slice(&[tag, 0, 0, 0]);
            raw.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            raw.extend_from_slice(payload);
        }
    }
    assert_eq!(raw.len(), 155);
    raw
}

pub(super) fn intent(case: &Case) -> NativeInsertIntent {
    NativeInsertIntent::decode(&wire(case, case.reverse), &mut budget(), &|| false).unwrap()
}
