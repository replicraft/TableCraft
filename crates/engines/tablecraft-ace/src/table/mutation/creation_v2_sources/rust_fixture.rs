//! SOURCE-only barrier and CI anchors for independently qualified Rust goldens.
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use tablecraft_testkit::access::canonical_creation::{CASES, Source, source};

fn checked_sources() -> [Source; 6] {
    let frozen_hashes = [
        "48afa7651c9a0bc8b24b4cbdf5bf3fd313f399daae9230cbec9e8a6719b580a9",
        "841b20aa2e2ce3c4ce17348312e6beae42aceb1af9f5e3f706827a22cbcedde7",
        "ff8fab1722063835c998833312b7bd8a514eaa9ca7926eca2fc2b4a88499a261",
        "93287f83d6b561d4585c508305490e66048ca5ccb57f0e58c1de16f1fa2405d8",
        "17bc957889d05764a2d3cc330139341856970aeac69331eda60341ba306295ff",
        "ea6d6b1637209cd16ec7dd0f22a439575424ad1194157a51b3c488cb8c5e23fa",
    ];
    let fixtures = CASES.map(source);
    for ((case, fixture), expected_hash) in CASES.into_iter().zip(&fixtures).zip(frozen_hashes) {
        let expected: [u8; 32] = std::array::from_fn(|index| {
            u8::from_str_radix(&expected_hash[2 * index..2 * index + 2], 16).unwrap()
        });
        let actual = Sha256::digest(&fixture.bytes);
        assert!(
            actual.iter().copied().eq(expected),
            "fixture SHA: {}",
            case.id()
        );
        assert_eq!(fixture.bytes.len(), 36_864);
    }
    fixtures
}

#[test]
fn canonical_source_hashes_remain_bound_to_sdk_qualified_originals() {
    let _ = checked_sources();
}

#[test]
#[ignore = "Explicit six-source fixture qualification before any production creator execution"]
fn qualify_rust_canonical_sources_before_production_creator() {
    let fixtures = checked_sources();
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_CANONICAL_CREATION_V2_SOURCE_DIR")
            .expect("explicit frozen Python source directory is required"),
    );
    assert!(directory.is_absolute());
    for (case, fixture) in CASES.into_iter().zip(&fixtures) {
        let path = directory.join(format!("canonical-v2-h5-{}.accdb", case.id()));
        assert_eq!(
            fs::read(path).unwrap(),
            fixture.bytes,
            "whole source: {}",
            case.id()
        );
    }

    // Every Rust source has already matched the exact independently authored,
    // SDK-observed original. Reuse the actual parent read-only gate on those
    // same immutable bytes, including full Metadata/GLOBAL/LVAL/map census.
    // It invokes no production constructor, edit, preview, save or journal.
    super::qualify_canonical_creation_v2_originals_without_mutation();

    for (case, fixture) in CASES.into_iter().zip(fixtures) {
        let path = directory.join(format!("canonical-v2-h5-{}.accdb", case.id()));
        assert_eq!(fs::read(path).unwrap(), fixture.bytes);
    }
}
