#[path = "calculated_storage/fixture.rs"]
mod calculated_fixture;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

use std::io::Cursor;
use tablecraft_ace::{
    InspectError, StoredInt32Calculation, read_stored_int32_calculation, read_table,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8]) -> Result<StoredInt32Calculation, InspectError> {
    read_stored_int32_calculation(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn explicit_stored_results_preserve_null_extremes_and_stale_values_without_query_eligibility() {
    let values = [
        Some(20),
        Some(-40),
        None,
        Some(i32::MIN),
        Some(i32::MAX),
        Some(41),
    ];
    for code in [5, 6] {
        let mut bytes = calculated_fixture::database(&values);
        bytes[20] = code;
        let stored = read(&bytes).unwrap();
        assert_eq!(stored.values(), values);
        assert_eq!(stored.column().name(), "Score");
        assert_eq!(
            stored
                .features()
                .columns()
                .nth(1)
                .unwrap()
                .expression()
                .unwrap(),
            Some("[Id]*2")
        );
        assert!(stored.column().scalar_type().is_none());
        assert_eq!(
            read_table(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
}

#[test]
fn unknown_or_malformed_wrappers_after_valid_rows_publish_no_snapshot() {
    let original = calculated_fixture::database(&[Some(20), Some(41)]);
    let second =
        u16::from_le_bytes(original[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap()) as usize;
    for offset in [0, 16, 24] {
        let mut bytes = original.clone();
        bytes[12 * 4096 + second + 6 + offset] ^= 1;
        assert_eq!(
            read(&bytes).err(),
            Some(if offset == 16 {
                InspectError::Malformed
            } else {
                InspectError::Unsupported
            })
        );
    }
}

#[test]
fn declarations_profiles_and_widths_are_required_even_on_empty_tables() {
    let original = calculated_fixture::database(&[]);
    assert!(read(&original).unwrap().values().is_empty());
    for (offset, value) in [
        (20, 2),
        (10 * 4096 + 63 + 25 + 16, 0x80),
        (10 * 4096 + 63 + 25 + 23, 38),
    ] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
    assert_eq!(
        read_stored_int32_calculation(
            &mut Cursor::new(&original),
            CatalogScope::new(1),
            "Items",
            "missing",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::UnknownColumn)
    );
    let mut missing = fixture::with_rule("Expression", "[Id]*2");
    missing[4..20].copy_from_slice(b"Standard ACE DB\0");
    missing[20] = 6;
    assert_eq!(read(&missing).err(), Some(InspectError::Unsupported));
    for (offset, value, expected) in [
        (3, 10, InspectError::Malformed),
        (8, 8, InspectError::Unsupported),
    ] {
        let mut bytes = original.clone();
        let property = bytes
            .windows(9)
            .position(|part| part == [9, 0, 0, 2, 1, 0, 1, 0, 4])
            .unwrap();
        bytes[property + offset] = value;
        assert_eq!(read(&bytes).err(), Some(expected));
    }
}

#[test]
fn stored_reads_retain_limits_and_cancellation() {
    let bytes = calculated_fixture::database(&[Some(20)]);
    for (mut allowance, cancelled) in [(budget(), true), (ResourceBudget::new(66, 1, 100), false)] {
        assert!(
            read_stored_int32_calculation(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut allowance,
                &|| cancelled
            )
            .is_err()
        );
    }
}
