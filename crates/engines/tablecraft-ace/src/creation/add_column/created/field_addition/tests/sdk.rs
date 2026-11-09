//! Literal hashes are independent SDK observations, not production constants.
use super::*;

#[test]
fn first_text_twins_match_eight_complete_sdk_images() {
    for (case, compressed, source_hash, after_hash) in [
        (
            raw::SourceCase::IdOnly,
            false,
            "b52d124892cdaab8fbe9c5028c876f57f1f82ee280cea7984aad68f1ead5eb13",
            "8ab534635f537de37676868858c276dafa465e78558a443c11a4e5b494a75b31",
        ),
        (
            raw::SourceCase::IdOnly,
            true,
            "b52d124892cdaab8fbe9c5028c876f57f1f82ee280cea7984aad68f1ead5eb13",
            "5a47a64973b8847ff174cd6f94da6fc7b6fc7eb159b74e8e044a647bc02ac6fb",
        ),
        (
            raw::SourceCase::InlineNumber,
            false,
            "9bb835b86e8e0789728834ae8871ab9b51d647c69bea89a091d15fd14cd8a90e",
            "e7783095ca422c8023060eea985405efc0397f0dd1af50d23c680baa8b8df07b",
        ),
        (
            raw::SourceCase::InlineNumber,
            true,
            "9bb835b86e8e0789728834ae8871ab9b51d647c69bea89a091d15fd14cd8a90e",
            "08b43874ba150d4d7db4cbf5af409f4ded5fde69577fa43e62d520886ca3be10",
        ),
        (
            raw::SourceCase::TwoNumberHistory,
            false,
            "8ca406b45272b6225d8f44b37ba74336250257cb5b37f0d074200d1837ef9ff9",
            "df22e93cb73d8e5a89428dfd28f65e3370d974f1b418e22deb6a553a8566a96f",
        ),
        (
            raw::SourceCase::TwoNumberHistory,
            true,
            "8ca406b45272b6225d8f44b37ba74336250257cb5b37f0d074200d1837ef9ff9",
            "21299f7253f2dbc22aab30e0b7989daf36b94645f5bee7cb2f9d7f5587a98fdd",
        ),
        (
            raw::SourceCase::ThreeNumberHistory,
            false,
            "337e03454d5aa233f66087301d5e8fe50092c4e90dfa4a65888430cdb9f61dd6",
            "17da7eb8c5e3513cb0f95a7f7b5d4b36e136297d375b3d44fcfb0511f75bae7f",
        ),
        (
            raw::SourceCase::ThreeNumberHistory,
            true,
            "337e03454d5aa233f66087301d5e8fe50092c4e90dfa4a65888430cdb9f61dd6",
            "628a1a8462a8ee971ffe3a72fffe94e72457ca39a760a0cd52cbb949272d18a6",
        ),
    ] {
        let before = raw::source(case);
        assert_eq!(hash(&before), source_hash);
        let expected = raw::append_text(&before, "Label", 255, compressed);
        qualify(
            &before,
            &expected,
            "Label",
            text(255, compressed),
            after_hash,
        );
    }
}

#[test]
fn second_text_preserves_compression_and_matches_four_complete_sdk_images() {
    for (case, after_hash) in [
        (
            raw::SourceCase::IdOnly,
            "6fad5df6b6d3809f69ffc3350396e0702cb349966d64661c0f8c5f02280fc71f",
        ),
        (
            raw::SourceCase::InlineNumber,
            "a6ff61144a4cc55c57b5fd69dbb1b269043610150a3e0f2c6083713b6522e960",
        ),
        (
            raw::SourceCase::TwoNumberHistory,
            "f2009440468a49da41d67a940278040ca5e462c0b36a607b23f1eb6af3f71e28",
        ),
        (
            raw::SourceCase::ThreeNumberHistory,
            "e92e4b083f17055936b34e23a7d0db84d681a78d7dbf44f5178ccbedc6d252a9",
        ),
    ] {
        let before = raw::append_text(&raw::source(case), "Label", 255, true);
        let expected = raw::append_text(&before, "OtherLabel", 255, true);
        qualify(
            &before,
            &expected,
            "OtherLabel",
            text(255, true),
            after_hash,
        );
    }
}

#[test]
fn number_after_text_preserves_variable_fields_and_matches_four_complete_sdk_images() {
    for (case, after_hash) in [
        (
            raw::SourceCase::IdOnly,
            "57600b1b190ee87fed1d858076b00fd6f5f83431adc1ff38788a4f952aa0eb98",
        ),
        (
            raw::SourceCase::InlineNumber,
            "c8382c58a3a87248230985e2690ab15d8a978e65daa6d230228c43fd493ab73f",
        ),
        (
            raw::SourceCase::TwoNumberHistory,
            "eec96c35704e9975ef2dbc0959cf697965b7bd0cf7029d58c16dece18049ce0b",
        ),
        (
            raw::SourceCase::ThreeNumberHistory,
            "41d0da72909b53a86250b70d37b88b096d13c34aa09ef72f8e1e31cd4dd088c7",
        ),
    ] {
        let before = raw::append_text(&raw::source(case), "Label", 255, true);
        let expected = raw::append_number(&before, "AfterLabel");
        qualify(
            &before,
            &expected,
            "AfterLabel",
            FieldAdditionTarget::NumberLongZero,
            after_hash,
        );
    }
}

#[test]
fn explicit_text_widths_match_two_complete_sdk_images() {
    let before = raw::source(raw::SourceCase::IdOnly);
    for (width, after_hash) in [
        (
            1,
            "06dd5d0cb676ba32fab564b8497e4ec5400d535a7b33a37cec86b5583d684067",
        ),
        (
            80,
            "1c7c7d6128fdbbcd752591a7672b6a9ece24c82c3dee5a00e1ed964c08ef0b4c",
        ),
    ] {
        let expected = raw::append_text(&before, "Label", width, true);
        qualify(
            &before,
            &expected,
            "Label",
            text(usize::from(width), true),
            after_hash,
        );
    }
}

#[test]
fn number_target_reuses_the_same_candidate_and_complete_third_sdk_pin() {
    let before = raw::source(raw::SourceCase::TwoNumberHistory);
    let expected = raw::append_number(&before, "Third");
    qualify(
        &before,
        &expected,
        "Third",
        FieldAdditionTarget::NumberLongZero,
        "337e03454d5aa233f66087301d5e8fe50092c4e90dfa4a65888430cdb9f61dd6",
    );
}

#[test]
fn third_text_after_number_matches_four_complete_sdk_images() {
    for (case, expected_hash) in [
        (
            raw::SourceCase::IdOnly,
            "fb5cf8454223e2a5513d4645d28a885823df76edf9852550afd173200155d356",
        ),
        (
            raw::SourceCase::InlineNumber,
            "4c133c3b9c0abd0dd84b7376de0bca6ea4c59a960bdda38696ea4593b76f6d01",
        ),
        (
            raw::SourceCase::TwoNumberHistory,
            "66235a4b2fea952cf793d2ea945f82f192bc7655f1ed8bfe3d96f9feebef5a10",
        ),
        (
            raw::SourceCase::ThreeNumberHistory,
            "b1a30352d573092e2ffd22cda254177efda351fbbcb38a70d259a92dbae85cc5",
        ),
    ] {
        let initial = raw::source(case);
        let label = raw::append_text(&initial, "Label", 255, true);
        let before = raw::append_number(&label, "AfterLabel");
        let after = raw::append_text(&before, "OtherLabel", 255, true);

        qualify(
            &before,
            &after,
            "OtherLabel",
            text(255, true),
            expected_hash,
        );
    }
}
