//! Independent literal hash pins copied from the retained SDK observation record.
use super::{SourceCase, append_number, append_text, source};
#[cfg(any(feature = "access-support", feature = "cli-support"))]
use sha2::{Digest, Sha256};

#[cfg(any(feature = "access-support", feature = "cli-support"))]
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(any(feature = "access-support", feature = "cli-support"))]
#[test]
fn all_eighteen_short_text_sdk_images_and_third_number_match_independent_hashes() {
    for (case, source_hash, uncompressed, compressed, repeated, number_after) in [
        (
            SourceCase::IdOnly,
            "b52d124892cdaab8fbe9c5028c876f57f1f82ee280cea7984aad68f1ead5eb13",
            "8ab534635f537de37676868858c276dafa465e78558a443c11a4e5b494a75b31",
            "5a47a64973b8847ff174cd6f94da6fc7b6fc7eb159b74e8e044a647bc02ac6fb",
            "6fad5df6b6d3809f69ffc3350396e0702cb349966d64661c0f8c5f02280fc71f",
            "57600b1b190ee87fed1d858076b00fd6f5f83431adc1ff38788a4f952aa0eb98",
        ),
        (
            SourceCase::InlineNumber,
            "9bb835b86e8e0789728834ae8871ab9b51d647c69bea89a091d15fd14cd8a90e",
            "e7783095ca422c8023060eea985405efc0397f0dd1af50d23c680baa8b8df07b",
            "08b43874ba150d4d7db4cbf5af409f4ded5fde69577fa43e62d520886ca3be10",
            "a6ff61144a4cc55c57b5fd69dbb1b269043610150a3e0f2c6083713b6522e960",
            "c8382c58a3a87248230985e2690ab15d8a978e65daa6d230228c43fd493ab73f",
        ),
        (
            SourceCase::TwoNumberHistory,
            "8ca406b45272b6225d8f44b37ba74336250257cb5b37f0d074200d1837ef9ff9",
            "df22e93cb73d8e5a89428dfd28f65e3370d974f1b418e22deb6a553a8566a96f",
            "21299f7253f2dbc22aab30e0b7989daf36b94645f5bee7cb2f9d7f5587a98fdd",
            "f2009440468a49da41d67a940278040ca5e462c0b36a607b23f1eb6af3f71e28",
            "eec96c35704e9975ef2dbc0959cf697965b7bd0cf7029d58c16dece18049ce0b",
        ),
        (
            SourceCase::ThreeNumberHistory,
            "337e03454d5aa233f66087301d5e8fe50092c4e90dfa4a65888430cdb9f61dd6",
            "17da7eb8c5e3513cb0f95a7f7b5d4b36e136297d375b3d44fcfb0511f75bae7f",
            "628a1a8462a8ee971ffe3a72fffe94e72457ca39a760a0cd52cbb949272d18a6",
            "e92e4b083f17055936b34e23a7d0db84d681a78d7dbf44f5178ccbedc6d252a9",
            "41d0da72909b53a86250b70d37b88b096d13c34aa09ef72f8e1e31cd4dd088c7",
        ),
    ] {
        let before = source(case);
        assert_eq!(hash(&before), source_hash, "{case:?} source");
        assert_eq!(
            hash(&append_text(&before, "Label", 255, false)),
            uncompressed,
            "{case:?} uncompressed"
        );
        let label = append_text(&before, "Label", 255, true);
        assert_eq!(hash(&label), compressed, "{case:?} compressed");
        assert_eq!(
            hash(&append_text(&label, "OtherLabel", 255, true)),
            repeated,
            "{case:?} repeated text"
        );
        assert_eq!(
            hash(&append_number(&label, "AfterLabel")),
            number_after,
            "{case:?} Number after Text"
        );
    }
    let before = source(SourceCase::IdOnly);
    assert_eq!(
        hash(&append_text(&before, "Label", 1, true)),
        "06dd5d0cb676ba32fab564b8497e4ec5400d535a7b33a37cec86b5583d684067"
    );
    assert_eq!(
        hash(&append_text(&before, "Label", 80, true)),
        "1c7c7d6128fdbbcd752591a7672b6a9ece24c82c3dee5a00e1ed964c08ef0b4c"
    );
}

#[test]
fn later_text_and_number_keep_prior_compression_widths_and_variable_ordinals() {
    let before = source(SourceCase::IdOnly);
    let label = append_text(&before, "Label", 255, true);
    let other = append_text(&label, "OtherLabel", 80, false);
    let number = append_number(&other, "AfterLabel");
    let page = &number[7 * 4096..8 * 4096];
    assert_eq!(u16::from_le_bytes(page[43..45].try_into().unwrap()), 2);
    assert_eq!(u16::from_le_bytes(page[45..47].try_into().unwrap()), 4);
    let old = &label[7 * 4096 + 100..7 * 4096 + 125];
    assert_eq!(
        &page[100..125],
        old,
        "old compressed field descriptor is literal"
    );
    assert_eq!(page[100], 10);
    assert_eq!(page[116], 1);
    assert_eq!(&page[107..109], &[0, 0]);
    assert_eq!(&page[123..125], &[254, 1]);
    assert_eq!(page[125], 10);
    assert_eq!(page[141], 0);
    assert_eq!(&page[132..134], &[1, 0]);
    assert_eq!(&page[148..150], &[160, 0]);
    assert_eq!(page[150], 4);
    assert_eq!(&page[157..159], &[2, 0]);
    assert_eq!(&page[171..173], &[4, 0]);
    assert_eq!(&page[173..175], &[4, 0]);
}

#[test]
fn every_old_external_property_slot_span_and_body_survives_mixed_appends() {
    let before = append_text(&source(SourceCase::TwoNumberHistory), "Label", 255, true);
    let after = append_number(&before, "AfterLabel");
    let old = &before[10 * 4096..11 * 4096];
    let new = &after[10 * 4096..11 * 4096];
    let rows = u16::from_le_bytes(old[12..14].try_into().unwrap()) as usize;
    assert_eq!(
        u16::from_le_bytes(new[12..14].try_into().unwrap()) as usize,
        rows + 1
    );
    for slot in 0..rows {
        let word = 14 + 2 * slot;
        assert_eq!(&old[word..word + 2], &new[word..word + 2]);
        let start = u16::from_le_bytes(old[word..word + 2].try_into().unwrap()) as usize;
        let end = if slot == 0 {
            4096
        } else {
            u16::from_le_bytes(old[word - 2..word].try_into().unwrap()) as usize
        };
        assert_eq!(&old[start..end], &new[start..end]);
    }
}

#[cfg(any(feature = "access-support", feature = "cli-support"))]
#[test]
fn text_number_text_matches_four_complete_third_step_sdk_images() {
    for (case, expected) in [
        (
            SourceCase::IdOnly,
            "fb5cf8454223e2a5513d4645d28a885823df76edf9852550afd173200155d356",
        ),
        (
            SourceCase::InlineNumber,
            "4c133c3b9c0abd0dd84b7376de0bca6ea4c59a960bdda38696ea4593b76f6d01",
        ),
        (
            SourceCase::TwoNumberHistory,
            "66235a4b2fea952cf793d2ea945f82f192bc7655f1ed8bfe3d96f9feebef5a10",
        ),
        (
            SourceCase::ThreeNumberHistory,
            "b1a30352d573092e2ffd22cda254177efda351fbbcb38a70d259a92dbae85cc5",
        ),
    ] {
        let before = source(case);
        let label = append_text(&before, "Label", 255, true);
        let number = append_number(&label, "AfterLabel");
        let after = append_text(&number, "OtherLabel", 255, true);

        assert_eq!(hash(&after), expected, "{case:?}");
    }
}
