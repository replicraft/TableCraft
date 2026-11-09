use super::*;
use crate::fixtures::access::canonical_created_number::{Source, mixed, repeated};
#[cfg(any(feature = "access-support", feature = "cli-support"))]
use crate::support::bytes::hex;
#[cfg(any(feature = "access-support", feature = "cli-support"))]
use sha2::{Digest, Sha256};

fn sources() -> Vec<Vec<u8>> {
    let number = repeated::source(Source::IdOnly);
    let text = mixed::append_text(
        &mixed::source(mixed::SourceCase::IdOnly),
        "Label",
        255,
        true,
    );
    let inline = mixed::append_text(
        &mixed::source(mixed::SourceCase::InlineNumber),
        "Label",
        255,
        true,
    );
    let text_number = mixed::append_number(&text, "AfterLabel");
    let text_number_text = mixed::append_text(&text_number, "OtherLabel", 255, true);
    vec![
        number.clone(),
        repeated::append(&number, "Score2"),
        text,
        inline,
        text_number_text,
    ]
}

#[cfg(any(feature = "access-support", feature = "cli-support"))]
#[test]
fn all_five_shared_column_images_match_independent_sdk_pins() {
    let pins = [
        "c88bc4cb1090d9b0c5e011010ec76cf9acc33da7ec9d58e67fa873bfaabe18ef",
        "6f9b095ddf3d724e34d5af98bea4e8ba88bc2b2bc70ade10814f85c27770c620",
        "47daf2169c931ee2e6545f61234ae227d8c02b166fd91c78d43d0efff2eadcd3",
        "e30ac2923f754892895a97231cee7cc6bd8f880c03f384a21546ed4b7e4b77f3",
        "c28d3c1bf1de4e75f894ac80ae332aa2a5b4b8c3c9111700d989222b8d6267c3",
    ];
    for (before, pin) in sources().into_iter().zip(pins) {
        let after = append_explicit_off_to_owned_page(&before);
        assert_eq!(hex(&Sha256::digest(&after)), pin);
    }
}

#[test]
fn shared_append_keeps_all_old_slots_bodies_maps_and_user_pages() {
    for before in sources() {
        let after = append_explicit_off_to_owned_page(&before);
        assert_eq!(after.len(), before.len());
        assert_eq!(&after[3 * PAGE..4 * PAGE], &before[3 * PAGE..4 * PAGE]);
        assert_eq!(&after[7 * PAGE..10 * PAGE], &before[7 * PAGE..10 * PAGE]);
        for page in [4 * PAGE, 10 * PAGE] {
            let count = word(&before, page + 12);
            let low = word(&before, page + 14 + 2 * (count - 1)) & 0x1fff;
            assert_eq!(
                &after[page + 14..page + 14 + 2 * count],
                &before[page + 14..page + 14 + 2 * count]
            );
            assert_eq!(
                &after[page + low..page + PAGE],
                &before[page + low..page + PAGE]
            );
            assert_eq!(word(&after, page + 12), count + 1);
        }
    }
}
