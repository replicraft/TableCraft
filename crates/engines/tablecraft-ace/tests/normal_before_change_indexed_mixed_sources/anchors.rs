//! First indexed mixed image and written-request pins, before producer calls.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_indexed_mixed_before_change::{Recipe, source};

const PINS: &[(u8, Recipe, &str, Option<&str>)] = &[
    (
        5,
        Recipe::ManualAscending,
        "c3072b510bbfee57bdc92dde0109fb50ba57faa660d81fc4e0d84c5c82670122",
        Some("5f0de007282fc66b4d6e56b7a2600bffee130e36d5f98ac7d7e4c51836e0b3c4"),
    ),
    (
        5,
        Recipe::CounterUniqueSwap,
        "7e03d18364ae23a5944d9bb80ac6c2ef0b8e40ce26353c3fe84216833f9c78e6",
        Some("1f8569b7e3e0780136c70d0dfb0c0e05a09719f6f7726b268951ace226bf3006"),
    ),
    (
        5,
        Recipe::CounterUniqueConflict,
        "7e03d18364ae23a5944d9bb80ac6c2ef0b8e40ce26353c3fe84216833f9c78e6",
        None,
    ),
    (
        5,
        Recipe::CounterCompositeTie,
        "c34416a3586a0ee173427447e260a704c3924e04a9ae1ef9a76e78a7fc71f3ea",
        Some("48bb897195d79ff587b2d38b69d7671687130682ede8564584bc10e3fea4f8a0"),
    ),
    (
        5,
        Recipe::CounterLeafPrefixLoss,
        "690f90428748c9967c0a022bb1bf1aefa24607c4728210d70d64f02fb258709d",
        Some("ab6b2d528f0d4aa87473ff1f80ec31858da3ffa142bd46dc24a65a92c7edd7a2"),
    ),
    (
        5,
        Recipe::CounterTreePrefixLoss,
        "3d30b4e9c88ca925c65ff0e5a577fdf334e04a2ceed825f8990db68f76dbb7b7",
        Some("f2485311eb412c3001c52ae7d43c35fa5efde9fe69d3aaea98f28aee11192395"),
    ),
    (
        5,
        Recipe::CounterTreeRetainedRestore,
        "a333a7ed1ebf9a062ed12447f539286af8c6f51967cf6933f261b6daaaf6bdfd",
        Some("a333a7ed1ebf9a062ed12447f539286af8c6f51967cf6933f261b6daaaf6bdfd"),
    ),
    (
        6,
        Recipe::ManualAscending,
        "93467452118b6ee96b7de5016286d5038012d553fcbc8e4ac3734faaee416327",
        Some("0fb59431198a384b52e6f4c98f8dec9257d5afd9686a7c80a44c4d8e84e35453"),
    ),
    (
        6,
        Recipe::CounterUniqueSwap,
        "b52913697f98569ca343c19f8a688bdccec7d2c4c26cbff10474f3c81b86d326",
        Some("b203906f0a6f02d9c5185945d8b60b9525cabc947a76f5ad27a845771a9af3f6"),
    ),
    (
        6,
        Recipe::CounterUniqueConflict,
        "b52913697f98569ca343c19f8a688bdccec7d2c4c26cbff10474f3c81b86d326",
        None,
    ),
    (
        6,
        Recipe::CounterCompositeTie,
        "231840ea85576c308e11769fec43e4e5475fab3e351e894b02db34dd75f4c0b1",
        Some("a996d48b7b87de07578d4896c0dda5757476a02b5dcf3cbffc9ddae4cbf8c4ed"),
    ),
    (
        6,
        Recipe::CounterLeafPrefixLoss,
        "368a8d57e4ce9f54ed2d15dc36ab85498c5707d984711c0cdfaf9464fb206674",
        Some("99e1e6c7812765a9801f5d318cac3164c751a24c4031c143064a5d6addca7f53"),
    ),
    (
        6,
        Recipe::CounterTreePrefixLoss,
        "21291f5da49265c93c037026fad0230b83d0c8ff39b5d58425da87a7b01e0c12",
        Some("ec6b3c64886e54935499031e39980bf936b8bf82f97db696f9a04172609bae97"),
    ),
    (
        6,
        Recipe::CounterTreeRetainedRestore,
        "7e6bec118c04570a996d038d01879815315e8f091719b0e1b46a915addc83234",
        Some("7e6bec118c04570a996d038d01879815315e8f091719b0e1b46a915addc83234"),
    ),
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[test]
fn indexed_mixed_sources_and_written_requests_keep_first_export_pins() {
    let mut requests = Sha256::new();
    let mut bytes = 0;

    for &(header, recipe, original, expected) in PINS {
        let case = source(header, recipe);
        assert_eq!(digest(&case.before), original);
        assert_eq!(case.expected.as_deref().map(digest).as_deref(), expected);
        bytes += case.before.len() + case.expected.as_ref().map_or(0, Vec::len);

        requests.update([header]);
        for text in [recipe.id(), "Items", "Score"] {
            requests.update((text.len() as u64).to_le_bytes());
            requests.update(text.as_bytes());
        }
        requests.update((case.updates.len() as u64).to_le_bytes());
        for update in &case.updates {
            requests.update((update.ordinal as u64).to_le_bytes());
            requests.update(update.expected.to_le_bytes());
            requests.update(update.replacement.to_le_bytes());
        }
    }

    assert_eq!(PINS.len(), 14);
    assert_eq!(bytes, 1220608);
    assert_eq!(
        hex(&requests.finalize()),
        "8364cc8aac752327a15760f050eee4631c970e0995604852ea9148eea4464486"
    );
}
