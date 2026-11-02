//! First untouched-NULL key source and written request pins, before producer calls.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_nullable_key_before_change::{Recipe, source};

const PINS: &[(u8, Recipe, &str, &str)] = &[
    (
        5,
        Recipe::ScalarAscending,
        "59a86ee4955f334cd0046c36971741127f1f810533451ca2b3dd2b229c65d711",
        "1a7e4b2cf87cbbc61f002a133a4b03a8f6d0dfa73a798689437d61558869881d",
    ),
    (
        5,
        Recipe::ScalarDescending,
        "abde8531cc09f980ec1a37c02c240f54aff095647f4bbb7a1f5d340e4a06619d",
        "db24f045052b5e791f10e81b5704a61f858744a24cb496e42bf1c37697ded592",
    ),
    (
        5,
        Recipe::UniqueNullPair,
        "fe085e9b57f2ea8138d66c9fcd027259ffd16a8df2121d6ed3504ee66f7c0748",
        "a0c15faa4cae387a1cf88bf5fee33f19a3839c0bbfb49b5ae74b0e3d966e268f",
    ),
    (
        5,
        Recipe::CompositePartialNull,
        "98f3b39601bff4a4154da2bceb08f12d7ef2c52aa6412d76fed347f71909d9f4",
        "09c1c81a6ab018114a3dc2da2af61b98c8d9b47782c25fe4055a5956512127eb",
    ),
    (
        5,
        Recipe::TreeNullFirst,
        "d73b44e18ae9c9bd69b6895539503f4ccd06481f2ed2cf872ae7ddf2f0ab7b13",
        "ea2d4039f050de40f95beb79ad41586fecc642b5d81088a51e71644295dee3ed",
    ),
    (
        5,
        Recipe::TreeNullBound,
        "432d9404d4400c6c839f0e02ee82b0f759ee3f5ca9d23098ca6799bd1e3d0375",
        "d7737b51b72d02c3af6938d53707e512f3cef31ee0a1911aa42ed4dbc30dcc9b",
    ),
    (
        6,
        Recipe::ScalarAscending,
        "966072c531bc638b8c62b93e1ae6115c6d25162c7badf5ef653aca82ae22b5a8",
        "80991d90afa0a18d18a86d9fc44bbb2dbcc42b741ef1fd26a19f25e51f27a169",
    ),
    (
        6,
        Recipe::ScalarDescending,
        "4ddde0d10b634c7281bf20677626405255ce2938f1cca3c576d8e2b01732da23",
        "e6cac637b2f94d2c1f8adb65f15ffe02cc23cdfd738d64d37b9215dac40284f7",
    ),
    (
        6,
        Recipe::UniqueNullPair,
        "3811219b165b0f7d4b7232ba9734638dea9831ea5cbcb0573fcb6b6f599c5752",
        "32f0fc46cde7865654564d669e1d2925c9cefe22e6812757cf1126496f0e80a3",
    ),
    (
        6,
        Recipe::CompositePartialNull,
        "23cb53039335df06879ffc4053b13abb18e531d6c418fa987e6430ae52a7fbbd",
        "f4cf9296c954e70f060f2d342cf32e61c6d0e90395d128def568ed156fa167de",
    ),
    (
        6,
        Recipe::TreeNullFirst,
        "3f4a3904db6d94b1c9444fa9922b6634191e9a86e380d4ac87f2d4e8132fe086",
        "ab9ddc9a6a418554f601e40708c3c1e5d0b2f5dcf7a89c2851739d34207cc8ca",
    ),
    (
        6,
        Recipe::TreeNullBound,
        "2dfd0696e0ca6774a7ebb187082ca3659e8ba934dd61608fb0f871e074c186ae",
        "46a8816f63d30db0f3cf9a24de800f25cef65c099cc800a65706616dda6d1963",
    ),
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[test]
fn nullable_key_sources_and_written_requests_keep_first_export_pins() {
    let mut requests = Sha256::new();
    let mut bytes = 0;

    for &(header, recipe, original, expected) in PINS {
        let case = source(header, recipe);
        assert_eq!(digest(&case.before), original);
        assert_eq!(digest(&case.expected), expected);
        bytes += case.before.len() + case.expected.len();

        requests.update([header]);
        for text in [recipe.id(), "Items", "Score"] {
            requests.update((text.len() as u64).to_le_bytes());
            requests.update(text.as_bytes());
        }
        requests.update(1_u64.to_le_bytes());
        requests.update((case.update.ordinal as u64).to_le_bytes());
        requests.update(case.update.expected.to_le_bytes());
        requests.update(case.update.replacement.to_le_bytes());
    }

    assert_eq!(PINS.len(), 12);
    assert_eq!(bytes, 1146880);
    assert_eq!(
        hex(&requests.finalize()),
        "891d8aac21e6d1f7a784675eb1ac73a014c54d0e73340036d33a36be9cd1e429"
    );
}
