//! CI anchors distinguish positive creation sources from the reader-only alias.
use sha2::{Digest, Sha256};
use tablecraft_testkit::access::canonical_variable_creation;

fn assert_hash(bytes: &[u8], expected: &str, id: &str) {
    assert_eq!(bytes.len(), 36_864);
    let frozen: [u8; 32] = std::array::from_fn(|index| {
        u8::from_str_radix(&expected[2 * index..2 * index + 2], 16).unwrap()
    });
    assert!(
        Sha256::digest(bytes).iter().copied().eq(frozen),
        "frozen variable source SHA: {id}"
    );
}

#[test]
fn variable_sources_and_separate_reader_control_retain_exported_hashes() {
    let positive = [
        (
            "empty",
            "5f14f557dc5e902bc2427e50604db2282c700c21f246db6ece6a4aca075f7aa0",
        ),
        (
            "null-empty",
            "fbddf39d7a7eda5102a9c95b2808a852b84973aeef6c9a165d08dd55ef8366c8",
        ),
        (
            "unicode",
            "43eede4f4ea36e541909c7d216c6b0dd97208af45414303687a6c5e2d86baecb",
        ),
        (
            "decimal-signs",
            "bd0a338a4649bbc38311e0807c93b058d7d30d00bdbdcdaff57816c18ed3444e",
        ),
        (
            "escaped-bom",
            "b51582f1ed884c5d6e73aa288047d02acc604209a120d9e817bd6e2f339aad7e",
        ),
    ];
    assert_eq!(canonical_variable_creation::CASES.len(), positive.len());
    for (case, (id, hash)) in canonical_variable_creation::CASES.into_iter().zip(positive) {
        assert_eq!(case.id(), id);
        let original = canonical_variable_creation::source(case);
        assert_hash(&original.bytes, hash, id);
    }

    // This sixth file is not added to CASES or production creation expectations.
    let alias = canonical_variable_creation::raw_bom_alias_source();
    assert_hash(
        &alias.bytes,
        "d9fa56b9222f164c1ed2222bcff98db8d1e42c04b98a0eec215e1ce44867e5f4",
        "raw-bom-alias reader control",
    );
}
