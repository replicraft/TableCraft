//! Ordinary CI anchor for exported IEEE originals, independent of producer code.
use sha2::{Digest, Sha256};
use tablecraft_testkit::access::canonical_fixed_float_creation;

#[test]
fn fixed_ieee_sources_remain_bound_to_exported_original_hashes() {
    let expected = [
        (
            "positive-subnormal",
            "31a7c3c8f4cf2fe5b8fdf27b9806f541b5e46de976c1f0ea179c04f341791a60",
        ),
        (
            "negative-subnormal",
            "ff3b401aa87a6a0d4f1f06d8f461297103f8f43808722baaa5ce8bc46511034a",
        ),
        (
            "infinity",
            "d1eb78dcdf3e85eff551a26e907b72a3c4685451a89345cd0201bf292eaae9cf",
        ),
        (
            "quiet-nan",
            "d1727b139e0248106da78248b530ecdac373ab708b4e310e5c98f58d4559bdc8",
        ),
        (
            "signalling-nan",
            "2934336e5a848e50b46b0e7112c698df87b98387abc43982738bc4b68049cf65",
        ),
    ];
    assert_eq!(canonical_fixed_float_creation::CASES.len(), expected.len());
    for (case, (id, expected_hash)) in canonical_fixed_float_creation::CASES
        .into_iter()
        .zip(expected)
    {
        assert_eq!(case.id(), id);
        let source = canonical_fixed_float_creation::source(case);
        assert_eq!(source.bytes.len(), 36_864);
        let frozen: [u8; 32] = std::array::from_fn(|index| {
            u8::from_str_radix(&expected_hash[2 * index..2 * index + 2], 16).unwrap()
        });
        assert!(
            Sha256::digest(&source.bytes).iter().copied().eq(frozen),
            "frozen IEEE source SHA: {id}"
        );
    }
}
