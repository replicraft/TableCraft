#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod composite;
use tablecraft_testkit::access as support;

use std::{
    collections::BTreeMap,
    io::{self, Cursor, Read, Seek, SeekFrom},
};
use tablecraft_ace::{
    IndexCheckStatus, IndexConstraintViolation, InspectError, NativeReadSource, check_table,
    inspect_table_indexes, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const COMPOSITE_INDEX_PAGE: usize = 14;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}

struct Counted {
    bytes: Cursor<Vec<u8>>,
    read_bytes: usize,
    payload_pages: BTreeMap<u32, usize>,
}

impl Counted {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            read_bytes: 0,
            payload_pages: BTreeMap::new(),
        }
    }
}

impl Read for Counted {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self.bytes.read(output)?;
        self.read_bytes += count;
        Ok(count)
    }
}

impl Seek for Counted {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl NativeReadSource for Counted {
    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        _budget: &mut ResourceBudget,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        if page.get(4..8) == Some(b"LVAL".as_slice()) {
            *self.payload_pages.entry(number).or_default() += 1;
        }
        Ok(())
    }
}

#[test]
fn complete_checks_decode_non_key_memo_payloads_once_instead_of_for_each_index() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let scope = CatalogScope::new(7);
    let mut previous = Counted::new(bytes.clone());
    let inspection =
        inspect_table_indexes(&mut previous, scope, "USysRibbons", &mut budget(), &|| {
            false
        })
        .unwrap();
    let sequential = read_table(&mut previous, scope, "USysRibbons", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert!(
        sequential
            .rows()
            .iter()
            .flatten()
            .any(|value| { matches!(value, Value::Text(text) if !text.is_empty()) })
    );
    let sequential_payload_pages = previous.payload_pages.clone();
    assert!(!sequential_payload_pages.is_empty());

    for index in inspection.indexes() {
        read_table_index(
            &mut previous,
            scope,
            "USysRibbons",
            index.name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }

    let mut projected = Counted::new(bytes.clone());
    let report = check_table(&mut projected, scope, "USysRibbons", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(report.rows(), sequential.rows().len());
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert!(projected.read_bytes < previous.read_bytes);
    assert_eq!(projected.payload_pages, sequential_payload_pages);
    assert!(
        previous.payload_pages.values().sum::<usize>()
            > projected.payload_pages.values().sum::<usize>()
    );

    let payload_page = *sequential_payload_pages.keys().next().unwrap() as usize;
    let mut malformed = bytes.clone();
    malformed[payload_page * PAGE_BYTES + 4] = 0;
    assert!(matches!(
        check_table(
            &mut Cursor::new(malformed),
            scope,
            "USysRibbons",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Malformed)
    ));
    assert_eq!(projected.bytes.into_inner(), bytes);
}

#[test]
fn gapped_reversed_composite_keys_keep_original_logical_positions() {
    for order in [vec![(3, false), (1, true)], vec![(1, false), (3, true)]] {
        let bytes = composite::database(
            6,
            &[4, 4, 4, 4],
            &order,
            &[
                vec![Some(1), Some(20), Some(999), Some(7)],
                vec![Some(2), Some(10), Some(-999), Some(8)],
                vec![Some(3), None, Some(999), Some(8)],
            ],
            0,
        );
        let report = check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.rows(), 3);
        assert_eq!(report.indexes()[0].status(), IndexCheckStatus::Checked);
    }
}

#[test]
fn selected_index_corruption_and_operational_failures_return_no_report() {
    let mut bytes = composite::database(
        6,
        &[4, 4, 4],
        &[(2, false), (1, true)],
        &[
            vec![Some(1), Some(5), Some(9)],
            vec![Some(2), Some(6), Some(10)],
        ],
        0,
    );
    assert!(matches!(
        check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
    assert!(
        check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut ResourceBudget::new(1, 100, 1_000_000),
            &|| false,
        )
        .is_err()
    );

    bytes[COMPOSITE_INDEX_PAGE * PAGE_BYTES] = 0;
    assert!(matches!(
        check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Malformed)
    ));
}

#[test]
fn gapped_constraint_keys_cannot_be_replaced_with_dense_output_positions() {
    for (rows, flags, expected) in [
        (
            vec![
                vec![Some(1), Some(7), Some(10), Some(9)],
                vec![Some(2), Some(7), Some(20), Some(9)],
            ],
            0x81,
            IndexConstraintViolation::DuplicateKey,
        ),
        (
            vec![vec![Some(1), Some(7), Some(10), None]],
            0x88,
            IndexConstraintViolation::NullKey,
        ),
    ] {
        let mut bytes = composite::database(6, &[4, 4, 4, 4], &[(3, true), (1, false)], &rows, 0);
        composite::index_flags(&mut bytes, flags);
        let error = check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .err();
        assert_eq!(error, Some(InspectError::IndexConstraint(expected)));
    }
}
