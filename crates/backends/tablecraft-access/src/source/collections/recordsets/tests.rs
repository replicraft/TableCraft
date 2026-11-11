use super::*;
use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_ace::NativeReadSource;
use tablecraft_core::values::Value;
use tablecraft_db::{MemberAvailability, RecordsetPresence, StoredCell};
use tablecraft_model::CatalogScope;
use tablecraft_query::Source;
use tablecraft_testkit::fixtures::access::{self as raw, attachment_modern, attachments};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}

struct Watched {
    bytes: Cursor<Vec<u8>>,
    children: Rc<Cell<usize>>,
    payload: Rc<Cell<usize>>,
}
impl NativeReadSource for Watched {}
impl Read for Watched {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        match self.bytes.position() as usize / 4096 {
            22 => self.children.set(self.children.get() + 1),
            23 => {
                self.payload.set(self.payload.get() + 1);
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
            _ => {}
        }
        self.bytes.read(target)
    }
}
impl Seek for Watched {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn declared_recordsets_are_metadata_only_and_preserve_four_parent_states_with_shared_rows() {
    let children = Rc::new(Cell::new(0));
    let payload = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watched {
            bytes: Cursor::new(attachment_modern::presence_cases(6)),
            children: Rc::clone(&children),
            payload: Rc::clone(&payload),
        },
        CatalogScope::new(4401),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    let columns = source.recordset_columns("Items").unwrap();
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].name(), "Score");
    let schema = columns[0].schema();
    assert_eq!(schema.columns().len(), 6);
    let data = schema
        .columns()
        .iter()
        .position(|column| column.name() == "FileData")
        .unwrap();
    let name = schema
        .columns()
        .iter()
        .position(|column| column.name() == "FileName")
        .unwrap();
    assert_eq!(schema.availability()[data], MemberAvailability::Uninspected);
    assert_eq!(
        children.get(),
        0,
        "declaration discovery cannot read child records"
    );
    let member = source
        .collection_members("Items")
        .unwrap()
        .iter()
        .position(|member| member.column().name() == "FileName")
        .unwrap();
    let mut held = Vec::new();
    source
        .scan_structured(
            "Items",
            &[0],
            &[],
            &[member],
            &[0],
            &mut |row, lists, records| {
                let parent = held.len();
                assert_eq!(row.get(0), Some(&Value::Int32((parent + 1) as i32)));
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].len(), [2, 1, 0, 0][parent]);
                assert_eq!(
                    records[0].presence(),
                    if parent == 3 {
                        RecordsetPresence::Unassigned
                    } else {
                        RecordsetPresence::Assigned
                    }
                );
                assert_eq!(lists.len(), 1);
                assert_eq!(lists[0].len(), records[0].len());
                for (child, record) in records[0].rows().iter().enumerate() {
                    assert_eq!(record[data], StoredCell::Uninspected);
                    let StoredCell::Value(value) = &record[name] else {
                        panic!("metadata name was not inspected");
                    };
                    assert_eq!(Some(value), lists[0].get(child));
                }
                held.push(Arc::clone(&records[0]));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(held.len(), 4);
    assert_eq!(
        held[0].rows()[0][name],
        StoredCell::Value(Value::Text("synthetic.bin".into()))
    );
    assert_eq!(held[0].rows()[1][name], StoredCell::Value(Value::Null));
    assert_eq!(
        held[1].rows()[0][name],
        StoredCell::Value(Value::Text(String::new()))
    );
    assert!(children.get() > 0);
    let reads = children.get();
    let mut parent = 0;
    source
        .scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, lists, records| {
                assert!(lists.is_empty());
                assert!(Arc::ptr_eq(&held[parent], &records[0]));
                parent += 1;
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(parent, 4);
    assert_eq!(children.get(), reads, "complete source caches are reused");
    assert_eq!(payload.get(), 0);
}

#[test]
fn structured_metadata_never_reads_file_data_but_explicit_payload_selection_still_does() {
    let mut descriptor = (0x4000_0000 | attachments::PAYLOAD.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend((23_u32 * 256).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut bytes = attachment_modern::with_first_data(6, &descriptor);
    let mut page = raw::data_page(0, &[(0, attachments::PAYLOAD.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    bytes[23 * 4096..24 * 4096].copy_from_slice(&page);
    let payload = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watched {
            bytes: Cursor::new(bytes),
            children: Rc::new(Cell::new(0)),
            payload: Rc::clone(&payload),
        },
        CatalogScope::new(4402),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    let mut parents = 0;
    source
        .scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, _lists, records| {
                assert_eq!(records.len(), 1);
                parents += 1;
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(parents, 3);
    assert_eq!(payload.get(), 0);
    let data = source
        .collection_members("Items")
        .unwrap()
        .iter()
        .position(|member| member.column().name() == "FileData")
        .unwrap();
    let mut callbacks = 0;
    assert_eq!(
        source.scan_structured(
            "Items",
            &[0],
            &[],
            &[data],
            &[0],
            &mut |_row, _lists, _records| {
                callbacks += 1;
                Ok(())
            }
        ),
        Err(QueryError::SourceFailure(SourceFailure::Io(
            std::io::ErrorKind::PermissionDenied
        )))
    );
    assert_eq!(callbacks, 0);
    assert_eq!(payload.get(), 1);
}

#[test]
fn malformed_child_suffix_and_caller_rejection_never_publish_partial_recordset_caches() {
    let mut bytes = attachment_modern::presence_cases(6);
    bytes[22 * 4096] = 0;
    let source = NativeSource::new(
        Cursor::new(bytes),
        CatalogScope::new(4403),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    assert_eq!(source.recordset_columns("Items").unwrap().len(), 1);
    let mut callbacks = 0;
    assert_eq!(
        source.scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, _lists, _records| {
                callbacks += 1;
                Ok(())
            }
        ),
        Err(QueryError::SourceFailure(SourceFailure::InvalidData)),
    );
    assert_eq!(callbacks, 0);
    let cache = source.entry("Items").unwrap().collections.get().unwrap();
    assert!(
        cache.recordsets.get().unwrap().groups[0]
            .rows
            .get()
            .is_none()
    );

    let source = NativeSource::new(
        Cursor::new(attachment_modern::presence_cases(6)),
        CatalogScope::new(4404),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    let mut callbacks = 0;
    assert_eq!(
        source.scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, _lists, _records| {
                callbacks += 1;
                Err(QueryError::Cancelled)
            }
        ),
        Err(QueryError::Cancelled)
    );
    assert_eq!(callbacks, 1);
    let mut parents = 0;
    source
        .scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, _lists, _records| {
                parents += 1;
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        parents, 4,
        "a rejected query prefix is not a complete relation"
    );
}

#[test]
fn required_index_failure_precedes_structured_parent_or_child_materialization() {
    let children = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watched {
            bytes: Cursor::new(attachment_modern::presence_cases(6)),
            children: Rc::clone(&children),
            payload: Rc::new(Cell::new(0)),
        },
        CatalogScope::new(4405),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections()
    .with_index("Items", "Missing")
    .unwrap();
    assert_eq!(source.recordset_columns("Items").unwrap().len(), 1);
    let mut callbacks = 0;
    assert_eq!(
        source.scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, _lists, _records| {
                callbacks += 1;
                Ok(())
            }
        ),
        Err(QueryError::UnknownName),
    );
    assert_eq!(callbacks, 0);
    assert_eq!(children.get(), 0);
}

#[test]
fn structured_source_budget_and_cancellation_fail_without_cached_partial_rows() {
    let stopped = Cell::new(false);
    let cancelled = || stopped.get();
    let source = NativeSource::new(
        Cursor::new(attachment_modern::presence_cases(6)),
        CatalogScope::new(4406),
        budget(),
        &cancelled,
    )
    .unwrap()
    .with_collections();
    assert_eq!(source.recordset_columns("Items").unwrap().len(), 1);
    *source.budget.try_borrow_mut().unwrap() = ResourceBudget::new(1, 100_000, 10_000_000);
    let mut callbacks = 0;
    assert!(matches!(
        source.scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, _lists, _records| {
                callbacks += 1;
                Ok(())
            }
        ),
        Err(QueryError::SourceFailure(SourceFailure::Limit(
            tablecraft_core::limits::LimitError {
                resource: Resource::Bytes,
                ..
            }
        )))
    ));
    assert_eq!(callbacks, 0);
    let cache = source.entry("Items").unwrap().collections.get().unwrap();
    assert!(
        cache.recordsets.get().unwrap().groups[0]
            .rows
            .get()
            .is_none()
    );
    *source.budget.try_borrow_mut().unwrap() = budget();
    stopped.set(true);
    assert_eq!(
        source.scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, _lists, _records| {
                callbacks += 1;
                Ok(())
            }
        ),
        Err(QueryError::Cancelled)
    );
    assert_eq!(callbacks, 0);
    stopped.set(false);
    source
        .scan_structured(
            "Items",
            &[0],
            &[],
            &[],
            &[0],
            &mut |_row, _lists, _records| {
                callbacks += 1;
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(callbacks, 4);
}
