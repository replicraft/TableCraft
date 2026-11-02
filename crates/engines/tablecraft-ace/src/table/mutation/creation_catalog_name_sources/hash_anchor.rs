//! Ordinary CI anchors for independent catalog-name originals, not repaired output.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_catalog_names;

#[test]
fn catalog_name_sources_retain_all_four_exported_original_hashes() {
    let expected = [
        (
            "table-bom",
            "a226d24ec4a843add4dc93e7571d9731117e91b0402ceda99f5bd689d8fcb371",
        ),
        (
            "table-mixed",
            "825b2a16cedd8fe68f40cbd150f4a387f856750388e74bda91eea14047c0ba00",
        ),
        (
            "table-boundary",
            "5766ad10a225a09f551673ef76c166d47bc95310a554bdf4acb5e3ae085b4d4c",
        ),
        (
            "column-raw",
            "dd97da58903e61a57fed467378743076781d24284e2924cb032039b182af9842",
        ),
    ];
    assert_eq!(canonical_catalog_names::CASES.len(), expected.len());
    for (case, (id, expected_hash)) in canonical_catalog_names::CASES.into_iter().zip(expected) {
        assert_eq!(case.id(), id);
        let source = canonical_catalog_names::source(case);
        assert_eq!(source.bytes.len(), 36_864);
        let frozen: [u8; 32] = std::array::from_fn(|index| {
            u8::from_str_radix(&expected_hash[2 * index..2 * index + 2], 16).unwrap()
        });
        assert!(
            Sha256::digest(&source.bytes).iter().copied().eq(frozen),
            "frozen catalog-name source SHA: {id}"
        );
    }
}
