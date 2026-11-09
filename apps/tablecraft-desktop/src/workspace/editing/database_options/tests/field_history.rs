//! The normal Options widgets use the same services after real field histories.
use super::assert_options_save_closes_persisted_tabs_and_preserves_scratch;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{Source, mixed, repeated},
    canonical_database_options,
};

#[test]
fn ordinary_options_save_after_number_and_mixed_text_histories_preserves_all_field_bytes() {
    let number = repeated::source(Source::IdOnly);
    let text = mixed::append_text(
        &mixed::source(mixed::SourceCase::IdOnly),
        "Label",
        255,
        true,
    );
    let text_number = mixed::append_number(&text, "AfterLabel");
    let mixed = mixed::append_text(&text_number, "OtherLabel", 255, true);

    for before in [number, mixed] {
        let expected = canonical_database_options::append_explicit_off_to_owned_page(&before);
        assert_options_save_closes_persisted_tabs_and_preserves_scratch(before, expected);
    }
}
