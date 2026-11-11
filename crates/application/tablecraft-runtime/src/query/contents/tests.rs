//! Issued metadata ownership is independent of query output position and native ROWs.
use super::*;
use crate::Response;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{MemberAvailability, RecordsetSchema, StoredCell};
use tablecraft_schema::{Column, NameComparison, ScalarType};
use tablecraft_testkit::fixtures::access::attachment_modern;

fn metadata() -> Arc<OwnedRecordset> {
    let mut work = ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX);
    let schema = Arc::new(
        RecordsetSchema::new(
            vec![
                Column::new("FileName".into(), ScalarType::Text, true).unwrap(),
                Column::new("FileData".into(), ScalarType::Binary, true).unwrap(),
            ],
            vec![
                MemberAvailability::Available,
                MemberAvailability::Uninspected,
            ],
            NameComparison::Exact,
            &mut work,
            &|| false,
        )
        .unwrap(),
    );
    Arc::new(
        OwnedRecordset::new(
            schema,
            vec![
                vec![
                    StoredCell::Value(Value::Text("metadata.bin".into())),
                    StoredCell::Uninspected,
                ],
                vec![StoredCell::Value(Value::Null), StoredCell::Uninspected],
            ],
            RecordsetPresence::Assigned,
            &mut work,
            &|| false,
        )
        .unwrap(),
    )
}

fn opened() -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>, Document) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("query-metadata.accdb");
    let bytes = attachment_modern::database(6);
    std::fs::write(&path, &bytes).unwrap();
    let (document, _) = Document::open(&path, &mut String::new(), 7, &|| false).unwrap();
    (directory, path, bytes, document)
}

#[test]
fn query_contents_bind_exact_result_and_issuer_without_native_parent_coordinates() {
    let (_directory, path, before, document) = opened();
    let value = metadata();
    let mut output = Presentation::new(&|| false);
    let mut builder = document.query_result_issuer().builder();
    builder.recordset_cell(4, 2, value, &mut output).unwrap();
    let result = builder.finish(&mut output).unwrap().unwrap();
    assert_eq!(
        result.bindings().collect::<Vec<_>>(),
        [QueryCollectionBinding {
            row: 4,
            column: 2,
            slot: 0
        }]
    );
    let Response::QueryContents {
        binding,
        columns,
        rows,
    } = document
        .inspect_query_collection(&result, 0, &|| false)
        .unwrap()
    else {
        panic!("issued metadata contents");
    };
    assert_eq!(binding, 0);
    assert_eq!(
        columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["FileName", "FileData"]
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text, "metadata.bin");
    assert!(matches!(
        rows[1][0].kind,
        CellKind::Scalar { is_null: true, .. }
    ));
    assert!(rows.iter().all(|row| row[1].kind == CellKind::Uninspected));
    assert!(matches!(
        document.inspect_query_collection(&result, 1, &|| false),
        Err(Error::QueryContentsUnavailable)
    ));
    assert!(matches!(
        document.inspect_query_collection(&result, 0, &|| true),
        Err(Error::Cancelled)
    ));
    let (different_issuer, _) = Document::open(&path, &mut String::new(), 7, &|| false).unwrap();
    assert!(matches!(
        different_issuer.inspect_query_collection(&result, 0, &|| false),
        Err(Error::StaleDocument)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn query_contents_share_snapshot_storage_and_reject_duplicate_output_bindings() {
    let (_directory, _path, _before, document) = opened();
    let snapshot = metadata();
    let shared = Arc::clone(&snapshot);
    let mut output = Presentation::new(&|| false);
    let mut builder = document.query_result_issuer().builder();
    builder.recordset_cell(0, 0, snapshot, &mut output).unwrap();
    builder.recordset_cell(0, 1, shared, &mut output).unwrap();
    assert!(matches!(
        builder.recordset_cell(0, 1, metadata(), &mut output),
        Err(Error::QueryContentsUnavailable)
    ));
    let result = builder.finish(&mut output).unwrap().unwrap();
    let same_owner = result.clone();
    assert!(result.shares_result(&same_owner));
    assert_eq!(result.bindings().len(), 2);
    let state = &result.state;
    assert!(Arc::ptr_eq(
        &state.bindings[0].value,
        &state.bindings[1].value
    ));
    let expected = size_of::<ResultState>()
        + ARC_COUNTERS
        + state.bindings.capacity() * size_of::<Binding>()
        + state.bindings[0].value.retained_bytes()
        + ARC_COUNTERS
        + state.bindings[0].value.schema().retained_bytes()
        + ARC_COUNTERS;
    assert_eq!(result.retained_bytes(), expected);
}
