//! Ordinary CI anchors for the independently exported complete capacity images.
use sha2::{Digest, Sha256};
use tablecraft_testkit::access::canonical_variable_capacity_creation;

#[test]
fn variable_capacity_originals_retain_two_exported_whole_image_hashes() {
    let expected = [
        (
            "maximum-rows",
            "afb4a9ddf125b05d58220311cc0306bdfca8eb39bb821d9a08ea3d62c72c76f8",
        ),
        (
            "null-tail",
            "1a4ceddc54dbd9fe114b8d96ada1ca6c64c5a21540e8991f73ad6927502bb7de",
        ),
    ];
    assert_eq!(
        canonical_variable_capacity_creation::CASES.len(),
        expected.len()
    );
    for (case, (id, hash)) in canonical_variable_capacity_creation::CASES
        .into_iter()
        .zip(expected)
    {
        assert_eq!(case.id(), id);
        // Check the complete literal before decoding its byte pairs.
        assert_eq!(
            hash.len(),
            64,
            "bind the actual frozen export SHA before registration"
        );
        let frozen: [u8; 32] = std::array::from_fn(|index| {
            u8::from_str_radix(&hash[2 * index..2 * index + 2], 16).unwrap()
        });
        let original = canonical_variable_capacity_creation::source(case);
        assert_eq!(original.source.bytes.len(), 36_864);
        assert!(
            Sha256::digest(&original.source.bytes)
                .iter()
                .copied()
                .eq(frozen),
            "frozen variable-capacity source SHA: {id}"
        );
    }
}
