//! Whole-image anchors for the independently read-qualified table sets.
use sha2::{Digest, Sha256};
use tablecraft_testkit::access::canonical_multiple_creation;

#[test]
fn multiple_table_originals_retain_four_exported_whole_image_hashes() {
    let expected = [
        (
            "two-empty",
            "184608b6af3eb6fe1d699d092e50bc0a40ec400a462e6dd2336c72b54e3fbc0e",
        ),
        (
            "populated",
            "e8e4daf3ef32b6d8dfbae4557f492b81296809ca253fe499cf5079b3a8e81aa8",
        ),
        (
            "candidate-bits",
            "3310ac8fc951ee1364a23fdb789e7f377d2522a391b6a7669250f7e58b1f8d93",
        ),
        (
            "sixteen",
            "5a2d6db36bce216f7cfc425f29218d168accdc9cfebb51bc55b74b389a721d3f",
        ),
    ];
    assert_eq!(canonical_multiple_creation::CASES.len(), expected.len());

    for (case, (id, hash)) in canonical_multiple_creation::CASES.into_iter().zip(expected) {
        assert_eq!(case.id(), id);
        let expected: [u8; 32] = std::array::from_fn(|index| {
            u8::from_str_radix(&hash[2 * index..2 * index + 2], 16).unwrap()
        });
        let source = canonical_multiple_creation::source(case);
        assert_eq!(Sha256::digest(&source.bytes).as_slice(), expected, "{id}");
    }
}
