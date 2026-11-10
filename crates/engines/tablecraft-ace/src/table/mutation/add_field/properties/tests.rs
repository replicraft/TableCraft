//! Raw fixture bytes independently pin existing-key order and dictionary growth.
use super::*;
use crate::inspect_table_properties;
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::{self as raw, Source, mixed};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}

fn inspect(bytes: &[u8], table: &str) -> TableProperties {
    inspect_table_properties(
        &mut Cursor::new(bytes),
        CatalogScope::new(1001),
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn dictionary_extension_and_new_group_match_independent_raw_number_models() {
    let empty = raw::source(Source::IdOnly);
    let default_only = raw::source(Source::DefaultOnly);
    let text = mixed::append_text(
        &mixed::source(mixed::SourceCase::IdOnly),
        "Label",
        255,
        true,
    );
    let existing = raw::source(Source::GuiRequiredZero);
    for (table, before, expected) in [
        (
            "Table1",
            &empty,
            raw::append(&empty, Source::IdOnly, "Bonus"),
        ),
        (
            "Items",
            &default_only,
            raw::append(&default_only, Source::DefaultOnly, "Bonus"),
        ),
        ("Table1", &text, mixed::append_number(&text, "Bonus")),
        (
            "Table1",
            &existing,
            raw::append(&existing, Source::GuiRequiredZero, "Bonus"),
        ),
    ] {
        let original = inspect(before, table);
        let independently_authored = inspect(&expected, table);
        let actual = append_number(&original, "Bonus", &mut budget(), &|| false).unwrap();
        assert_eq!(actual, independently_authored.raw_bytes());
        assert_eq!(
            body_bytes(&original, 5, Target::Number, &mut budget(), &|| false).unwrap(),
            actual.len()
        );
        assert_eq!(
            body_bytes(&original, 0, Target::Number, &mut budget(), &|| false).unwrap() + 10,
            actual.len()
        );
        if !original.raw_bytes().is_empty() {
            let old_end =
                4 + u32::from_le_bytes(original.raw_bytes()[4..8].try_into().unwrap()) as usize;
            let new_end = 4 + u32::from_le_bytes(actual[4..8].try_into().unwrap()) as usize;
            assert_eq!(&actual[10..old_end], &original.raw_bytes()[10..old_end]);
            let old_groups = &original.raw_bytes()[old_end..];
            assert_eq!(&actual[new_end..new_end + old_groups.len()], old_groups);
        }
    }
}
