//! First mixed source export pins, independent of normal writer output.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_mixed_before_change::{Recipe, source};

const PINS: &[(u8, Recipe, &str, Option<&str>)] = &[
    (
        5,
        Recipe::FixedSequential,
        "df9bd60fc28aa39208e7ec8e5a3677bc3063c55c389ffb76ad7ac7dd3bd3179f",
        Some("b323bd18b2cb568e1d966d5579bc9d72d40beddf62f578de318ced70a15a1b8c"),
    ),
    (
        5,
        Recipe::FixedRestore,
        "4a732827ccf338ffc8128fe4ac32e1327e154e4f1108b54d4ed604d9585e2acc",
        Some("4a732827ccf338ffc8128fe4ac32e1327e154e4f1108b54d4ed604d9585e2acc"),
    ),
    (
        5,
        Recipe::VariableManual,
        "86e3f7c9cf16f4b3123bda0f0b985f8add39c67e5a253747ae2c87f5e722e152",
        Some("76046040bf6815ffd4a19617d68fca8109ed02d9ee77c4b764e2ac08a73f068c"),
    ),
    (
        5,
        Recipe::CounterUnicode,
        "c1a1af06cb52115f3168a0fff1990e64b66c91ce39372b7a88a26ac89b8bb733",
        Some("9526e28c0e95f9912644e69d1d86a10e54d70e5c2464274df8313e5475a605e6"),
    ),
    (
        5,
        Recipe::CounterNullEmpty,
        "c6c93811fc65f50d01011054f46599f2a875b978ab7f1223c190d43e1737d70d",
        Some("334d21934471de03e278f922cd14a69a849a49dd5211072cadf8cc345001f773"),
    ),
    (
        5,
        Recipe::CounterPrimary,
        "8fce432aa1f7414ecf360ad3a905031193b312ff851d70084c562b30ef2789c3",
        Some("cafa3f9b22d330a7b0780c7496d37446e93f4642d417c4afd0df9b73e2779d73"),
    ),
    (
        5,
        Recipe::CounterRestore,
        "574961913a19a146e91640401346b3061ff24ae77dfcfe9374338b903ff7f1e5",
        Some("574961913a19a146e91640401346b3061ff24ae77dfcfe9374338b903ff7f1e5"),
    ),
    (
        5,
        Recipe::CounterEmpty,
        "8fce432aa1f7414ecf360ad3a905031193b312ff851d70084c562b30ef2789c3",
        Some("8fce432aa1f7414ecf360ad3a905031193b312ff851d70084c562b30ef2789c3"),
    ),
    (
        5,
        Recipe::LateRaise,
        "b592cad95d2c15b488a81bd518b3fe7a36a770cd68cdc92b98651476e56afa47",
        None,
    ),
    (
        5,
        Recipe::DormantCounter,
        "89f7cdb0330f83c7a770a69d088f739dcff6de46c7bdbb61af714a9d27823ff0",
        None,
    ),
    (
        5,
        Recipe::UserCounter,
        "8fce432aa1f7414ecf360ad3a905031193b312ff851d70084c562b30ef2789c3",
        None,
    ),
    (
        5,
        Recipe::OpaqueOperand,
        "e77aafccaa82511991b68a2720ce27ff72596ef20eaff8d20d3ecf5b08804663",
        None,
    ),
    (
        6,
        Recipe::FixedSequential,
        "381c421f891a8f7fe71fb68a1e4613282c2fead4cc264f735f7cd163206465c3",
        Some("d25718c32fc9dd20d5265347fd5e56a900f3e7d27bc5c2dc5cb21a1d01e7369e"),
    ),
    (
        6,
        Recipe::FixedRestore,
        "5959de0efb4df98d2b861c4b207ee1de2759319f64e024ee8abe68389aff6856",
        Some("5959de0efb4df98d2b861c4b207ee1de2759319f64e024ee8abe68389aff6856"),
    ),
    (
        6,
        Recipe::VariableManual,
        "4c480a23ee0c189a70ff3ec6448ecb12335a074ae2d1a1bdabf541a7e49f1b11",
        Some("94e82abdedc9590c3e0a757bbecc92a6895ed3718afab494fed99e2febe8f626"),
    ),
    (
        6,
        Recipe::CounterUnicode,
        "1b4c1e364770f1316c9d3dc88b0c358fb4f82d0ff21ef4b8e54ac3059294af1b",
        Some("80f2b41fb08535859afa3c406ad9facc44dac4c51811ff8ae624a303202f2007"),
    ),
    (
        6,
        Recipe::CounterNullEmpty,
        "e941b57a241afc92a6031db28df1ac2f063026e7d3f78b388ed03f8ed6d60529",
        Some("4b0a2965f5668ed01d30810faf8d68165224c167704ae07a8f7ba87b6622cfb5"),
    ),
    (
        6,
        Recipe::CounterPrimary,
        "2d1e1eec880720d7b7319e9fcebaaed56d4229f4d49228373e172df92161e90e",
        Some("fab501a04bfb51f21d07f075b4eee227fa9bd52bd779def6c9f76ed1e5f1555e"),
    ),
    (
        6,
        Recipe::CounterRestore,
        "50d8f27d58a620cda2c7f46ea0a54638fa22b6909cb4a99c35327482c813991b",
        Some("50d8f27d58a620cda2c7f46ea0a54638fa22b6909cb4a99c35327482c813991b"),
    ),
    (
        6,
        Recipe::CounterEmpty,
        "2d1e1eec880720d7b7319e9fcebaaed56d4229f4d49228373e172df92161e90e",
        Some("2d1e1eec880720d7b7319e9fcebaaed56d4229f4d49228373e172df92161e90e"),
    ),
    (
        6,
        Recipe::LateRaise,
        "26f9cace462c8dffb12add8c1bcb7fb6b6c3d891eb1487c4a14472f332030b7e",
        None,
    ),
    (
        6,
        Recipe::DormantCounter,
        "f4458a7b5dee4245340d4a848975109f559cc825c9305b2f02541157aabf223b",
        None,
    ),
    (
        6,
        Recipe::UserCounter,
        "2d1e1eec880720d7b7319e9fcebaaed56d4229f4d49228373e172df92161e90e",
        None,
    ),
    (
        6,
        Recipe::OpaqueOperand,
        "ec8e7a583cef23132af44f87cddb0e0fe0d02855297c81d6f17934b8b7af30af",
        None,
    ),
];

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn mixed_sources_and_original_requests_keep_first_export_pins() {
    let mut requests = Sha256::new();
    let mut bytes = 0;

    for &(header, recipe, original, expected) in PINS {
        let case = source(header, recipe);
        assert_eq!(digest(&case.before), original);
        assert_eq!(case.expected.as_deref().map(digest).as_deref(), expected);
        bytes += case.before.len() + case.expected.as_ref().map_or(0, Vec::len);

        requests.update([header]);
        for text in [recipe.id(), case.column] {
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

    assert_eq!(PINS.len(), 24);
    assert_eq!(bytes, 1556480);
    let actual = requests.finalize();
    assert_eq!(
        digest_bytes(&actual),
        "e3d5b2d451c35bbded3e4ab6bacf9e08817006028146fb013831067f6cb9986a"
    );
}

fn digest_bytes(raw: &[u8]) -> String {
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}
