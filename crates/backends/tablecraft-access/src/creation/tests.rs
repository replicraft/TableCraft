use super::*;
use std::fs;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::canonical_creation::{self, Case};
use tablecraft_testkit::access::canonical_native17_creation as native17;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn shared_creation_service_publishes_six_independently_qualified_complete_images() {
    for case in canonical_creation::CASES {
        let mut source = canonical_creation::source(case);
        source.bytes = native17::source(source.bytes);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("new.accdb");
        let destination = CreationDestination::capture(&path, &mut budget(), &|| false).unwrap();
        let columns: Vec<_> = source.columns.iter().map(String::as_str).collect();
        let rows: Vec<_> = source.rows.iter().map(Vec::as_slice).collect();
        let prepared = destination
            .prepare(source.table, &columns, &rows, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            (prepared.row_count(), prepared.column_count()),
            (source.rows.len(), source.columns.len())
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        let receipt = prepared.publish(&mut budget(), &|| false).unwrap();
        assert!(receipt.namespace_synced());
        assert_eq!(
            (receipt.row_count(), receipt.column_count()),
            (source.rows.len(), source.columns.len())
        );
        assert_eq!(fs::read(path).unwrap(), source.bytes);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn invalid_typed_rows_create_no_staging_or_final_entry() {
    let mut source = canonical_creation::source(Case::One);
    source.bytes = native17::source(source.bytes);
    for rows in [vec![vec![1]], vec![vec![1, 10]; 65]] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("new.accdb");
        let destination = CreationDestination::capture(&path, &mut budget(), &|| false).unwrap();
        let columns: Vec<_> = source.columns.iter().map(String::as_str).collect();
        let rows: Vec<_> = rows.iter().map(Vec::as_slice).collect();
        let error = destination
            .prepare(source.table, &columns, &rows, &mut budget(), &|| false)
            .err()
            .expect("invalid rows must not create output");
        assert!(matches!(error, CreationError::Create(_)));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

mod edges;
mod typed;

mod variable;

mod multiple;
