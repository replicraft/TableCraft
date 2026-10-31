//! First-export pins, frozen before retained-tree normal writer execution.
use sha2::{Digest, Sha256};
use tablecraft_testkit::access::canonical_tree_before_change::{Recipe, source};

const PINS: &[(u8, Recipe, &str, &str)] = &[
    (
        5,
        Recipe::LeafPrefix,
        "c1220648698b434f19fb9256b312e6d281a22fcffd5a085604955fbcbb585181",
        "35181db597d4f2fac8165fe73cce60ba37bfd9a3b7d1bf91c699bce6ae5eceb0",
    ),
    (
        5,
        Recipe::NodeAscending,
        "e1209bb55ebaa0d30f13d2be19cb33bdbff25780ad96dd6399c3f61b703fdf91",
        "ea6697c38519b32462200b62282afdf6342df7abc0b45db98c6d80e210377ffc",
    ),
    (
        5,
        Recipe::NodeDescendingPrefix,
        "4ddfe584c49c1cb5af399fd696eae2c70bb11f15009a8248cff2ef3b7a8e86a1",
        "ac4384ae2a2172510eccdf4dd6f9c3b8d3bc6fbbeb37b2e33ecbcf16f3ebba08",
    ),
    (
        5,
        Recipe::NodeCompositeMacro,
        "fab825710f0eccaaea186ca854af77513ec340986bb9a031479f263f0f061945",
        "781d8a959b19419be6ae75451329a4075273c0dc7fb746db9ef2b8cd1802eec7",
    ),
    (
        5,
        Recipe::NodeUniqueSwap,
        "365024741c0345143e5dff9291d44aa9982d1650946f7fdca5d8f0d40769cb8a",
        "2abed5a9585bd772643d95d2b9ebc64493d7188eaad99071db79c6a1bc339705",
    ),
    (
        5,
        Recipe::LeafPrefixLoss,
        "c1220648698b434f19fb9256b312e6d281a22fcffd5a085604955fbcbb585181",
        "5bc06142b92ca5456c3fa470b341bc9528ed0b1a43566df9abdc0e1a29f4ef53",
    ),
    (
        5,
        Recipe::NodePrefixLoss,
        "8ca86ef463e0f8cf78d2ffcb1b994f8e7a8480e0224ef1b426de78b894da361d",
        "7dd7d2a2d1d2fbe3bc10ee5cfd9753168e24d1b2bc44c1387d9b600aec456832",
    ),
    (
        5,
        Recipe::NodeZeroTail,
        "7d0032d940d9dd6d1477380e25773dffc38988ee440e7e71eca730d88d015b73",
        "fe79650c58ef14bf4934d1e34218b1a5782035464307fac55cb6efadd5470019",
    ),
    (
        5,
        Recipe::NodeRetainedUpper,
        "367f85f4e13699e7dc7c55ce0888f532aed21701b088f43c75ca81cc380e87dd",
        "367f85f4e13699e7dc7c55ce0888f532aed21701b088f43c75ca81cc380e87dd",
    ),
    (
        6,
        Recipe::LeafPrefix,
        "8a653d7b6376266c23e609565474d62502eb28a0d64856b408465afffe580563",
        "7433324d8ad06789bd4fc9cf63fde9eb9e656ea9a0e923cb85221d0b5873a957",
    ),
    (
        6,
        Recipe::NodeAscending,
        "d3e8b099fccac1dccf4e614cb26542b58c8a2a1a36b62792cb4a7cf91f36e4b9",
        "fc71e86c7939671b9ee31fe9ad09ed8b04f6b7d9a7ca6e0ae339699b50cf36f5",
    ),
    (
        6,
        Recipe::NodeDescendingPrefix,
        "66ca796e685b79f2c5d191ee5077cc57c369104e7338b1592a8c0e15884aae34",
        "ca775ab1ce3bbe62efa2212503558235facc4c4812ddb7aeadccd5c9e85491e3",
    ),
    (
        6,
        Recipe::NodeCompositeMacro,
        "0d9a63d22ef2ddb981b81bc3d227a6045aa6b1d73855f50ce9e9d5986881691d",
        "9db697a71a45a1ba9bf35839820a42ba8f9e5164568234559190ff660599840d",
    ),
    (
        6,
        Recipe::NodeUniqueSwap,
        "60bf44737c7d1875d8d04df4247714a9336c4e3ba0e9ca502d5f3d9f68c4a865",
        "924afab21e55177f82fac4b21d0de6a00f97aa4fe44d8f7663b23fb2deceb55b",
    ),
    (
        6,
        Recipe::LeafPrefixLoss,
        "8a653d7b6376266c23e609565474d62502eb28a0d64856b408465afffe580563",
        "ed5ebf759d73ac904da1fab44deffac068b5b05540b123b40a4e7196671466d6",
    ),
    (
        6,
        Recipe::NodePrefixLoss,
        "8007a898d860e742138c61247d993f8ca9f07ce7579f494522077bcd3c7c1999",
        "113617535dcc1074916272b7e1835cb03ffa4eecc1283d50feaa13a703616843",
    ),
    (
        6,
        Recipe::NodeZeroTail,
        "9b2bc16a25a453377312787c5970814422730e53f541b203761164c29b13c392",
        "3e2ca906e504f2a05bac5eb4119b8e39c062b8ef14d1cf27d2676396bb9ff7a1",
    ),
    (
        6,
        Recipe::NodeRetainedUpper,
        "05e6ae91df7103393ade36b4b09f25643f882f65f2d6428f8a107b6863fd46a5",
        "05e6ae91df7103393ade36b4b09f25643f882f65f2d6428f8a107b6863fd46a5",
    ),
];

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn retained_tree_sources_outcomes_and_original_requests_keep_first_export_pins() {
    let mut requests = Sha256::new();
    let mut bytes = 0;
    for &(header, recipe, original, expected) in PINS {
        let case = source(header, recipe);
        assert_eq!(digest(&case.before), original);
        assert_eq!(digest(&case.expected), expected);
        bytes += case.before.len() + case.expected.len();

        let name = recipe.id().as_bytes();
        requests.update([header]);
        requests.update((name.len() as u64).to_le_bytes());
        requests.update(name);
        requests.update((case.updates.len() as u64).to_le_bytes());
        for update in &case.updates {
            requests.update((update.ordinal as u64).to_le_bytes());
            requests.update(update.expected.to_le_bytes());
            requests.update(update.replacement.to_le_bytes());
        }
    }
    assert_eq!(PINS.len(), 18);
    assert_eq!(bytes, 1_703_936);
    let actual = requests
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        actual,
        "6708c8478277fd2d1f7f706698923668650d434744626a765a714194918f287b"
    );
}
