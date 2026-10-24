#[path = "calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "calculated_storage/scalar_fixture.rs"]
mod scalar_fixture;
mod support;

use std::io::{Cursor, Read, Seek, SeekFrom};
use tablecraft_ace::{
    InspectError, NativeReadSource, StoredCalculation, TablePageRequest,
    inspect_calculation_declaration, read_stored_calculation, read_stored_table_page, read_table,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const FIELD_START: usize = 10 * 4096 + 63 + 25;
const NUMERIC_PRECISION_OFFSET: usize = 11;
const NUMERIC_SCALE_OFFSET: usize = 12;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

fn payload() -> Vec<u8> {
    vec![14, 0, 4, 128, 0, 0, 0, 0, 57, 48, 0, 0, 0, 0, 0, 0]
}

fn database(code: u8, values: &[Option<Vec<u8>>]) -> Vec<u8> {
    scalar_fixture::database(code, 16, 39, false, values)
}

fn read(bytes: &[u8]) -> Result<StoredCalculation, InspectError> {
    read_stored_calculation(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn cached_decimals_keep_effective_scale_separate_from_raw_declared_parameters() {
    for code in [5, 6] {
        let bytes = database(code, &[Some(payload()), None, Some(vec![])]);
        let stored = read(&bytes).unwrap();
        assert_eq!(stored.data_type(), ScalarType::Decimal);
        assert_eq!(stored.column().decimal_parameters(), Some((0, 0)));
        assert_eq!(stored.column().scalar_type(), None);
        let expected = Value::Decimal(Decimal::new(-12345, 4).unwrap());
        assert_eq!(
            stored.values(),
            [expected.clone(), Value::Null, Value::Null]
        );
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

        let page = read_stored_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 4).unwrap(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(page.reached_end());
        assert_eq!(page.is_stored_calculation(1), Some(true));
        assert_eq!(page.columns()[1].data_type(), ScalarType::Decimal);
        assert_eq!(page.rows()[0], [Value::Int32(1), expected]);
        assert_eq!(
            read_table(
                &mut Cursor::new(&bytes),
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
fn empty_tables_still_require_exact_observed_numeric_declarations() {
    let bytes = database(6, &[]);
    assert!(read(&bytes).unwrap().values().is_empty());
    for (offset, value) in [
        (20, 2),
        (FIELD_START + 23, 43),
        (FIELD_START + 16, 0xc1),
        (FIELD_START + NUMERIC_PRECISION_OFFSET, 28),
        (FIELD_START + NUMERIC_SCALE_OFFSET, 4),
    ] {
        let mut changed = bytes.clone();
        changed[offset] = value;
        assert_eq!(read(&changed).err(), Some(InspectError::Unsupported));
    }
}

#[test]
fn malformed_late_numeric_values_do_not_publish_a_partial_snapshot() {
    let mut cases = vec![vec![0; 15], vec![0; 17]];
    for (position, value) in [(0, 0), (1, 1), (2, 29), (3, 1)] {
        let mut changed = payload();
        changed[position] = value;
        cases.push(changed);
    }
    cases.push(vec![
        14, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    ]);
    for malformed in cases {
        let bytes = database(6, &[Some(payload()), Some(malformed)]);
        assert!(read(&bytes).is_err());
    }
}

struct RecordsForbidden(Cursor<Vec<u8>>);

impl NativeReadSource for RecordsForbidden {}

impl Read for RecordsForbidden {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let start = self.0.position();
        let end = start.saturating_add(bytes.len() as u64);
        if start < 13 * 4096 && end > 12 * 4096 {
            return Err(std::io::Error::other(
                "Metadata inspection read stored records",
            ));
        }
        self.0.read(bytes)
    }
}

impl Seek for RecordsForbidden {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(position)
    }
}

#[test]
fn decimal_declaration_inspection_stays_row_free_over_invalid_cached_bytes() {
    let bytes = database(6, &[Some(vec![255])]);
    let declaration = inspect_calculation_declaration(
        &mut RecordsForbidden(Cursor::new(bytes)),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(declaration.data_type(), ScalarType::Decimal);
    assert_eq!(declaration.expression().unwrap(), "[Id]*2");
    assert_eq!(declaration.column().decimal_parameters(), Some((0, 0)));
}

#[test]
fn cached_decimal_reads_keep_native_budget_and_cancellation_failures() {
    let bytes = database(6, &[Some(payload())]);
    let mut denied = ResourceBudget::new(0, 0, 0);
    assert!(
        read_stored_calculation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut denied,
            &|| false,
        )
        .is_err()
    );
    assert_eq!(
        read_stored_calculation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| true,
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert_eq!(bytes, database(6, &[Some(payload())]));
}
