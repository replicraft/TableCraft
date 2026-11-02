//! Literal157-row outcomes qualified by root native Source8 and six inert SDK reads.
//! Source qualification does not establish a matching producer or native Access save pass.
use super::genuine_wide_root_promotion::{self, Case};
use std::{collections::BTreeMap, fs, path::PathBuf};

const PINS: &[(&str, usize, &str)] = &[
    (
        "h5-CurrencyKeys-compressed-halves.accdb",
        774144,
        "9b8762c1de2e051b987ac2e17595ba111449dd740b2c0fb121dd5ee93b60b01b",
    ),
    (
        "h5-Int64Keys-compressed-halves.accdb",
        774144,
        "8f15e2b794348965642e680a2d845600f086a766ce28449621623ad7feeef96c",
    ),
    (
        "h5-before.accdb",
        757760,
        "8e976662f1c2ddf2e7616870082a71da3a618017ab0cca6faf78f7fe301350a6",
    ),
    (
        "h6-CurrencyKeys-compressed-halves.accdb",
        843776,
        "d44a0bfe6197b64c0ad3047cb0a726f11da27a93eea7f1b173b2659dc79e7a25",
    ),
    (
        "h6-Int64Keys-compressed-halves.accdb",
        843776,
        "ec3db7f6b2b1236a9f2c42b5dae10889e18a9868ca47aa562514efc8332c5416",
    ),
    (
        "h6-before.accdb",
        827392,
        "dcb7f14d9eab21dd9f518ddd3aafdc90b06f9325a20b5bbe60cdc44303f39dd7",
    ),
];

/// Pin every immutable literal source before publishing an actual batch case.
/// These are independent expected images; this function establishes no writer permission.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_LEAF_COMPRESSION_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    verify(
        &fs::read(directory.join("manifest.json")).unwrap(),
        "829bc8ff6f0f1a33c5b52e214cd81ee60bf22a879c18c471333a1c59a59ab97c",
    );
    let mut files = BTreeMap::new();
    for &(name, length, expected) in PINS {
        let raw = fs::read(directory.join(name)).unwrap();
        assert_eq!(raw.len(), length);
        verify(&raw, expected);
        assert!(files.insert(name, raw).is_none());
    }
    assert_eq!(files.len(), 6);
    genuine_wide_root_promotion::from_images(
        157,
        |header| files[format!("h{header}-before.accdb").as_str()].clone(),
        |header, table| {
            files[format!("h{header}-{table}-compressed-halves.accdb").as_str()].clone()
        },
    )
}
