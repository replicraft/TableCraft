//! Independent TCND framing; the shared source loader has no engine dependency.
use super::budget;
use sha2::{Digest, Sha256};
use tablecraft_ace::{NativeDeleteIntent, RowDelete};
use tablecraft_testkit::fixtures::access::genuine_indexed_delete as sources;
pub(super) use tablecraft_testkit::fixtures::access::genuine_indexed_delete::{Case, Recipe};

pub(super) fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let digest = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(digest, expected);
    })
}

pub(super) fn requests(case: &Case, reverse: bool) -> Vec<RowDelete> {
    sources::witnesses(case.recipe, reverse)
        .into_iter()
        .map(|(ordinal, raw)| RowDelete::new(ordinal, raw))
        .collect()
}

pub(super) fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let requests = sources::witnesses(case.recipe, reverse);
    let total: u32 = match requests.len() {
        1 => 100,
        2 => 127,
        3 => 154,
        _ => panic!("closed source request width"),
    };
    let mut raw = Vec::from(*b"TCND0001");
    raw.extend_from_slice(&total.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((case.before.len() as u64).to_le_bytes());
    hash.update(&case.before);
    raw.extend_from_slice(&hash.finalize());
    for number in [5_u32, requests.len() as u32, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    for (ordinal, record) in requests {
        raw.extend_from_slice(&(ordinal as u64).to_le_bytes());
        raw.extend_from_slice(&15_u32.to_le_bytes());
        raw.extend_from_slice(&record);
    }
    assert_eq!(raw.len(), total as usize);
    raw
}

pub(super) fn intent(case: &Case, reverse: bool) -> NativeDeleteIntent {
    NativeDeleteIntent::decode(&wire(case, reverse), &mut budget(), &|| false).unwrap()
}
