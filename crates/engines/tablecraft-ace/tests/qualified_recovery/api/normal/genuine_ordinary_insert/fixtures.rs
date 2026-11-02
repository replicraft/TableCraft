//! Complete source images and independent TCNA request framing.
use super::budget;
use sha2::{Digest, Sha256};
use tablecraft_ace::{NativeInsertIntent, RowInsert};
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert as sources;
pub(super) use tablecraft_testkit::fixtures::access::genuine_ordinary_insert::Case;

pub(super) fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

pub(super) fn requests() -> Vec<RowInsert> {
    vec![RowInsert::new(sources::values())]
}

pub(super) fn wire(case: &Case) -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&115_u32.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    raw.extend_from_slice(&digest.finalize());
    for number in [8_u32, 1, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(b"Sentinel");
    raw.extend_from_slice(&39_u32.to_le_bytes());
    raw.extend_from_slice(&3_u32.to_le_bytes());
    for (tag, payload) in [(2, &[24, 0, 0, 0][..]), (6, &b"new"[..]), (0, &[][..])] {
        raw.extend_from_slice(&[tag, 0, 0, 0]);
        raw.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        raw.extend_from_slice(payload);
    }
    assert_eq!(raw.len(), 115);
    raw
}

pub(super) fn intent(case: &Case) -> NativeInsertIntent {
    NativeInsertIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}
