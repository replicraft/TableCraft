use super::{FIELD_BYTES, FIELD_START, PAGE_BYTES, TEMPLATE_PAGE, budget, database, read};
use std::io::Cursor;
use tablecraft_ace::{
    InspectError, inspect_version_history_declarations, read_version_history_column,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const OWNER_PAGE: usize = 10;
const FLAT_PAGE: usize = 20;
const VISIBLE_MEMO: usize = 2;
const FLAGS: usize = 15;
const EXTENDED_FLAGS: usize = 16;

fn compressed(profile: u8) -> Vec<u8> {
    let mut bytes = database();
    bytes[20] = profile;
    bytes[OWNER_PAGE * PAGE_BYTES + FIELD_START + VISIBLE_MEMO * FIELD_BYTES + EXTENDED_FLAGS] = 1;
    bytes
}

#[test]
fn compressed_current_memo_keeps_existing_history_names_values_and_metadata() {
    for profile in [2, 5, 6] {
        let bytes = compressed(profile);
        let declarations = inspect_version_history_declarations(
            &mut Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(declarations.len(), 2);
        let history = read(&bytes, "Notes").unwrap();
        assert_eq!(history.memo_column().extended_flags(), 1);
        let versions = history
            .stored_versions(0)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(versions[0].value(), Some("é🦀\0older"));
        assert_eq!(versions[1].value(), Some("newer"));
        assert_eq!(history.stored_versions(2).unwrap().len(), 0);
    }
}

#[test]
fn unknown_visible_flags_and_unqualified_child_compression_remain_refused() {
    for flags in [2, 3, 0x10, 0x11, 0xc0, 0xc1] {
        let mut bytes = compressed(5);
        bytes
            [OWNER_PAGE * PAGE_BYTES + FIELD_START + VISIBLE_MEMO * FIELD_BYTES + EXTENDED_FLAGS] =
            flags;
        assert_eq!(read(&bytes, "Notes").err(), Some(InspectError::Unsupported));
    }
    for flags in [0x03, 0x06] {
        let mut bytes = compressed(5);
        bytes[OWNER_PAGE * PAGE_BYTES + FIELD_START + VISIBLE_MEMO * FIELD_BYTES + FLAGS] = flags;
        assert_eq!(read(&bytes, "Notes").err(), Some(InspectError::Unsupported));
    }
    for (page, position) in [(TEMPLATE_PAGE, 0), (FLAT_PAGE, 1)] {
        let mut bytes = compressed(5);
        bytes[page * PAGE_BYTES + FIELD_START + position * FIELD_BYTES + EXTENDED_FLAGS] = 1;
        assert_eq!(read(&bytes, "Notes").err(), Some(InspectError::Unsupported));
    }
}

#[test]
fn compressed_memo_history_retains_source_limits_and_cancellation() {
    let original = compressed(6);
    for mut limits in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16_384, 0),
    ] {
        let mut source = Cursor::new(original.clone());
        assert!(matches!(
            read_version_history_column(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Notes",
                &mut limits,
                &|| false,
            ),
            Err(InspectError::Limit(_))
        ));
        assert_eq!(source.into_inner(), original);
    }
    assert_eq!(
        read_version_history_column(
            &mut Cursor::new(original),
            CatalogScope::new(1),
            "Items",
            "Notes",
            &mut budget(),
            &|| true,
        )
        .err(),
        Some(InspectError::Cancelled)
    );
}
