use super::{
    CatalogScope, Cursor, FIELD_START, InspectError, TablePageRequest, Value, budget,
    calculated_fixture, database, inline, inspect_calculation_declaration, read,
    read_stored_table_page, utf16, wrapped,
};

const FLAGS: usize = 15;
const EXTENDED_FLAGS: usize = 16;

fn hyperlink(profile: u8, values: &[Option<Vec<u8>>]) -> Vec<u8> {
    let mut bytes = database(profile, false, values, &[]);
    bytes[FIELD_START + FLAGS] = 0x82;
    bytes
}

#[test]
fn calculated_hyperlink_memo_keeps_raw_cached_text_and_explicit_metadata() {
    for profile in [5, 6] {
        let text = "Display#https://example.invalid/a#section#é🦀";
        let bytes = hyperlink(
            profile,
            &[
                Some(inline(&wrapped(&utf16(text)))),
                None,
                Some(inline(&wrapped(&[]))),
            ],
        );
        let stored = read(&bytes).unwrap();
        assert_eq!(stored.column().flags(), 0x82);
        assert_eq!(stored.column().extended_flags(), 0xc0);
        assert_eq!(
            stored.values(),
            [
                Value::Text(text.into()),
                Value::Null,
                Value::Text(String::new())
            ]
        );
        let page = read_stored_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 4).unwrap(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(page.is_stored_calculation(1), Some(true));
        assert_eq!(page.rows()[0][1], Value::Text(text.into()));
    }
}

#[test]
fn hyperlink_flag_does_not_admit_compressed_memo_unknown_or_fixed_flags() {
    for flags in [0x80, 0x83, 0x86, 0xff] {
        let mut bytes = hyperlink(6, &[]);
        bytes[FIELD_START + FLAGS] = flags;
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
    let mut numeric = calculated_fixture::database(&[]);
    numeric[FIELD_START + FLAGS] = 0x82;
    assert_eq!(read(&numeric).err(), Some(InspectError::Unsupported));

    for extended in [0, 1, 0xc1, 0xc2, 0xc3] {
        let mut bytes = hyperlink(6, &[]);
        bytes[FIELD_START + EXTENDED_FLAGS] = extended;
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
}

#[test]
fn hyperlink_calculation_metadata_stays_row_free_and_late_bad_wrappers_fail() {
    let mut bad = wrapped(&utf16("late"));
    bad[0] = 1;
    let bytes = hyperlink(
        5,
        &[Some(inline(&wrapped(&utf16("valid")))), Some(inline(&bad))],
    );
    let declaration = inspect_calculation_declaration(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(declaration.column().flags(), 0x82);
    assert!(read(&bytes).is_err());
}
