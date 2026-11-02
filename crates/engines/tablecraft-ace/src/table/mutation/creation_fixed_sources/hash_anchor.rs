//! Ordinary CI anchor for the five originals frozen before generic creation.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_fixed_creation;

#[test]
fn fixed_creation_sources_remain_bound_to_exported_original_hashes() {
    let expected = [
        (
            "empty",
            "101e5ca7b573e75bf788ddacd222a4d204773757105cb7c57887f97067809d24",
        ),
        (
            "endpoints",
            "2ec4a35526a1d573643731d5b7708496281a7fae3aad53b0abf5aacf5d8d6458",
        ),
        (
            "nullable",
            "d102fb5bd3202439bf2c36c424e5ad15aa96a27484bab0349f6374650e845c03",
        ),
        (
            "signed-zero",
            "2d595e2fea39b4b0cc513dd0bbc1066e7d24399299fd3f616d8dd6e98f3e4fc4",
        ),
        (
            "boundary",
            "c0c49db3cb8b1bbe2fc62fa14ff41247e93ef1c616a59d6defc259464a14727b",
        ),
    ];
    assert_eq!(canonical_fixed_creation::CASES.len(), expected.len());
    for (case, (id, expected_hash)) in canonical_fixed_creation::CASES.into_iter().zip(expected) {
        assert_eq!(case.id(), id);
        let source = canonical_fixed_creation::source(case);
        assert_eq!(source.bytes.len(), 36_864);
        let bytes: [u8; 32] = std::array::from_fn(|index| {
            u8::from_str_radix(&expected_hash[2 * index..2 * index + 2], 16).unwrap()
        });
        assert!(
            Sha256::digest(&source.bytes).iter().copied().eq(bytes),
            "frozen fixed source SHA: {id}"
        );
    }
}
