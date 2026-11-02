//! Frozen source and original-request pins, recorded before writer execution.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_nullable_index_id_before_change::source;

const PINS: &[(u8, &str, &str)] = &[
    (
        5,
        "707c74bf4945d648dffa17bf4459ec20ef15e939258cccebf80694ca7d3e7d16",
        "f51ff4159dbec253982c477960a5acba6af9cffd073ea5fdb299bad2b6e23c96",
    ),
    (
        6,
        "dfa1c6c7454ffc7d63f406f1ad1ba0939b7bc5562d8548a2c83a98cb274b7cbf",
        "b23dfbf57fae35012b735e6baeba4c4b071811ca2e6a03db1028c6e59b444bd2",
    ),
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn present_id_sources_and_original_requests_keep_first_export_pins() {
    let mut requests = Sha256::new();
    let mut bytes = 0;

    for &(header, original, expected) in PINS {
        let case = source(header);
        assert_eq!(hex(&Sha256::digest(&case.before)), original);
        assert_eq!(hex(&Sha256::digest(&case.expected)), expected);
        bytes += case.before.len() + case.expected.len();

        requests.update([header]);
        for text in ["present-id", "Items", case.column] {
            requests.update((text.len() as u64).to_le_bytes());
            requests.update(text.as_bytes());
        }
        requests.update(1_u64.to_le_bytes());
        requests.update((case.update.ordinal as u64).to_le_bytes());
        requests.update(case.update.expected.to_le_bytes());
        requests.update(case.update.replacement.to_le_bytes());
    }

    assert_eq!(bytes, 163840);
    assert_eq!(
        hex(&requests.finalize()),
        "905ed64f46af8325a8a7459053cbdf4f7f32894369ac7b7f65fbb0a1a6203c73"
    );
}
