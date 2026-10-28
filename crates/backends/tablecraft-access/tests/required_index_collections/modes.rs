//! Required-index joins for decoded files and stored Memo histories.
use super::*;

#[allow(dead_code)]
#[path = "../../../../engines/tablecraft-ace/tests/version_history/fixture.rs"]
mod history_fixture;
#[path = "modes_fixture.rs"]
mod modes_fixture;

fn lists(source: &dyn Source, member: &str) -> Vec<(Value, Vec<Value>)> {
    let declarations = source.collection_members("Items").unwrap();
    let position = declarations
        .iter()
        .position(|field| field.column().name() == member)
        .unwrap();
    let mut result = Vec::new();
    source
        .scan_collection_members("Items", &[0], &[], &[position], &mut |row, fields| {
            result.push((
                row.get(0).unwrap().clone(),
                (0..fields[0].len())
                    .map(|index| fields[0].get(index).unwrap().clone())
                    .collect(),
            ));
            Ok(())
        })
        .unwrap();
    result
}

fn expected_lists(
    descending: bool,
    populated: Vec<Value>,
    empty: Vec<Value>,
) -> Vec<(Value, Vec<Value>)> {
    let absent = (Value::Int32(10), vec![]);
    let present = (Value::Int32(10), empty);
    let unassigned = (Value::Int32(20), vec![]);
    let populated = (Value::Int32(30), populated);
    if descending {
        vec![populated, unassigned, absent, present]
    } else {
        vec![absent, present, unassigned, populated]
    }
}

#[test]
fn required_index_decoded_files_keep_null_empty_and_cached_root_alignment() {
    for code in [5, 6] {
        for descending in [false, true] {
            let blocked = Rc::new(Cell::new(None));
            let child_reads = Rc::new(Cell::new(0));
            let source = NativeSource::new(
                Watch {
                    bytes: Cursor::new(modes_fixture::decoded(code, descending)),
                    blocked: blocked.clone(),
                    child_reads: child_reads.clone(),
                    payload_reads: Rc::new(Cell::new(0)),
                },
                SCOPE,
                budget(),
                &|| false,
            )
            .unwrap()
            .with_collections()
            .with_index("Items", "IdOrder")
            .unwrap()
            .with_decoded_attachments();
            let expected = expected_lists(
                descending,
                vec![Value::Binary(vec![0, 1, 2, 255]), Value::Null],
                vec![Value::Binary(vec![])],
            );
            assert_eq!(lists(&source, "FileData"), expected);
            let reads = child_reads.get();
            assert!(reads > 0);
            blocked.set(Some(22));
            assert_eq!(lists(&source, "FileData"), expected);
            assert_eq!(child_reads.get(), reads);
            assert_eq!(
                visit(&source, &Cell::new(0)).unwrap(),
                super::expected(descending)
            );
        }
    }
}

#[test]
fn required_index_stored_memo_histories_keep_duplicate_ids_and_cache_alignment() {
    for code in [5, 6] {
        for descending in [false, true] {
            let blocked = Rc::new(Cell::new(None));
            let child_reads = Rc::new(Cell::new(0));
            let source = NativeSource::new(
                Watch {
                    bytes: Cursor::new(modes_fixture::history(code, descending)),
                    blocked: blocked.clone(),
                    child_reads: child_reads.clone(),
                    payload_reads: Rc::new(Cell::new(0)),
                },
                SCOPE,
                budget(),
                &|| false,
            )
            .unwrap()
            .with_collections()
            .with_index("Items", "IdOrder")
            .unwrap();
            let expected = expected_lists(
                descending,
                vec![Value::Text("older".into()), Value::Null],
                vec![Value::Text(String::new())],
            );
            assert_eq!(lists(&source, "Notes"), expected);
            let reads = child_reads.get();
            assert!(reads > 0);
            blocked.set(Some(22));
            assert_eq!(lists(&source, "Notes"), expected);
            assert_eq!(child_reads.get(), reads);
            let times = expected_lists(
                descending,
                vec![
                    Value::DateTime(
                        tablecraft_core::values::LegacyDateTime::from_bits(0.25_f64.to_bits())
                            .unwrap(),
                    ),
                    Value::Null,
                ],
                vec![Value::DateTime(
                    tablecraft_core::values::LegacyDateTime::from_bits(1.0_f64.to_bits()).unwrap(),
                )],
            );
            assert_eq!(lists(&source, "Modified"), times);
            assert_eq!(child_reads.get(), reads);
        }
    }
}
