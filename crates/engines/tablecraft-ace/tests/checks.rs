use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus as S, InspectError, check_table, read_table_index};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn copy() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn put16(b: &mut [u8], at: usize, value: u16) {
    b[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn completed_checks_report_all_index_coverage_without_claiming_skipped_profiles() {
    let bytes = copy();
    let mut source = Cursor::new(bytes.clone());
    let report = check_table(
        &mut source,
        CatalogScope::new(7),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.schema().name(), "ROULETTE_TB");
    assert_eq!(report.rows(), 38);
    assert_eq!(
        report
            .indexes()
            .iter()
            .map(|i| (i.index().name(), i.status()))
            .collect::<Vec<_>>(),
        vec![
            ("COLOR_ID", S::Checked),
            ("Number", S::Checked),
            ("ORDR_NBR", S::Checked)
        ]
    );
    assert_eq!(source.into_inner(), bytes);
}

#[test]
fn extra_live_data_roots_are_found_even_when_a_complete_index_read_succeeds() {
    let mut bytes = copy();
    // Independent candidate facts: 38 roots on data page 728, 4 KiB pages.
    let base = 728 * 4096;
    let last = usize::from(
        u16::from_le_bytes(
            bytes[base + 14 + 37 * 2..base + 16 + 37 * 2]
                .try_into()
                .unwrap(),
        ) & 0x1fff,
    );
    let end = usize::from(
        u16::from_le_bytes(
            bytes[base + 14 + 36 * 2..base + 16 + 36 * 2]
                .try_into()
                .unwrap(),
        ) & 0x1fff,
    );
    let raw = bytes[base + last..base + end].to_vec();
    let start = last - raw.len();
    assert!(start > 14 + 39 * 2);
    bytes[base + start..base + last].copy_from_slice(&raw);
    put16(&mut bytes, base + 14 + 38 * 2, start as u16);
    put16(&mut bytes, base + 12, 39);
    assert_eq!(
        read_table_index(
            &mut Cursor::new(&bytes),
            CatalogScope::new(7),
            "ROULETTE_TB",
            "ORDR_NBR",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows()
        .len(),
        38
    );
    assert_eq!(
        check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(7),
            "ROULETTE_TB",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::Malformed)
    );
}

#[test]
fn selected_failure_returns_no_report_but_unqualified_pages_stay_unverified() {
    let mut bytes = copy();
    bytes[731 * 4096] = 0;
    assert_eq!(
        check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(7),
            "ROULETTE_TB",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::Malformed)
    );
    let mut bytes = copy();
    bytes[730 * 4096] = 0;
    // Unknown text sort variant keeps this index explicitly unqualified.
    bytes[726 * 4096 + 124 + 13] = 1;
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.indexes()[1].status(), S::Unsupported);
}

#[test]
fn cumulative_rows_and_cancellation_are_checked_before_publication() {
    // Five complete catalog reads, one sequential table and three qualified indexes.
    let row_work = 5 * 146 + 4 * 38;
    assert!(
        check_table(
            &mut Cursor::new(copy()),
            CatalogScope::new(7),
            "ROULETTE_TB",
            &mut ResourceBudget::new(16 * 1024 * 1024, row_work, 1_000_000),
            &|| false
        )
        .is_ok()
    );
    assert!(matches!(
        check_table(
            &mut Cursor::new(copy()),
            CatalogScope::new(7),
            "ROULETTE_TB",
            &mut ResourceBudget::new(16 * 1024 * 1024, row_work - 1, 1_000_000),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(
        check_table(
            &mut Cursor::new(copy()),
            CatalogScope::new(7),
            "ROULETTE_TB",
            &mut budget(),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled)
    );
}
