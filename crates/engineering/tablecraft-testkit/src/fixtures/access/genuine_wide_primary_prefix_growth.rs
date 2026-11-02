//! Qualified atomic two-row primary-prefix outcomes; no producer bytes as oracle.
use super::genuine_wide_root_promotion::{self, Case};
use std::{collections::BTreeMap, fs, path::PathBuf};

const PINS: &[(&str, usize, &str)] = &[
    (
        "h5-CurrencyKeys-prefix3.accdb",
        774144,
        "da4d11ca75022cf98c2144dc6ca0639ebdedf29c5dd7cbf72d5741d2c53da64d",
    ),
    (
        "h5-Int64Keys-prefix3.accdb",
        774144,
        "51144d7dec0f57d3c9d135820fc7e273a110f7e746e084a4117740c0099d2b26",
    ),
    (
        "h5-before.accdb",
        757760,
        "8e976662f1c2ddf2e7616870082a71da3a618017ab0cca6faf78f7fe301350a6",
    ),
    (
        "h6-CurrencyKeys-prefix3.accdb",
        843776,
        "a22a538c11aacebad2a5ceec9ca8f5449424804c952ea4d9bfad51f7574b1805",
    ),
    (
        "h6-Int64Keys-prefix3.accdb",
        843776,
        "1d0426993172c85c58caf1fcba03cb9049fede2a2b56c3f85ddb7af5848d9c4f",
    ),
    (
        "h6-before.accdb",
        827392,
        "dcb7f14d9eab21dd9f518ddd3aafdc90b06f9325a20b5bbe60cdc44303f39dd7",
    ),
];

/// Verify every literal file before publishing a matching case.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_PRIMARY_PREFIX_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    verify(
        &fs::read(directory.join("manifest.json")).unwrap(),
        "566dc85244be008343498fae728f7c0d29e5093bf455b2493065add3bdf4ce75",
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
        2,
        |header| files[format!("h{header}-before.accdb").as_str()].clone(),
        |header, table| files[format!("h{header}-{table}-prefix3.accdb").as_str()].clone(),
    )
}
