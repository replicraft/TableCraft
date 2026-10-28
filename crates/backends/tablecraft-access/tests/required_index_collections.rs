#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "required_index_collections/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachment_modern/fixture.rs"]
mod modern_fixture;
#[path = "required_index_collections/modes.rs"]
mod modes;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachments/fixture.rs"]
mod old_attachment;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, NativeReadSource, read_complex_column_links,
    read_table_index_with_references,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

const SCOPE: CatalogScope = CatalogScope::new(1);
const PAYLOAD: usize = 27;

#[derive(Debug, PartialEq)]
struct Parent {
    id: i32,
    names: Vec<Value>,
    types: Vec<Value>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

fn expected(descending: bool) -> Vec<Parent> {
    let absent = || Parent {
        id: 10,
        names: vec![],
        types: vec![],
    };
    let empty_value = || Parent {
        id: 10,
        names: vec![Value::Text(String::new())],
        types: vec![Value::Text(String::new())],
    };
    let empty_parent = || Parent {
        id: 20,
        names: vec![],
        types: vec![],
    };
    let populated = || Parent {
        id: 30,
        names: vec![Value::Text("synthetic.bin".into()), Value::Null],
        types: vec![Value::Text("field/type".into()), Value::Null],
    };
    if descending {
        vec![populated(), empty_parent(), absent(), empty_value()]
    } else {
        vec![absent(), empty_value(), empty_parent(), populated()]
    }
}

fn visit(source: &dyn Source, callbacks: &Cell<usize>) -> Result<Vec<Parent>, QueryError> {
    let declarations = source.collection_members("Items")?;
    let names = declarations
        .iter()
        .position(|member| member.column().name() == "FileName")
        .unwrap();
    let types = declarations
        .iter()
        .position(|member| member.column().name() == "FileType")
        .unwrap();
    assert!(names < types);
    let mut result = Vec::new();
    source.scan_collection_members("Items", &[0], &[], &[names, types], &mut |row, lists| {
        callbacks.set(callbacks.get() + 1);
        let Value::Int32(id) = row.get(0).unwrap() else {
            panic!("expected native Id")
        };
        assert_eq!(lists.len(), 2);
        assert_eq!(lists[0].len(), lists[1].len());
        result.push(Parent {
            id: *id,
            names: (0..lists[0].len())
                .map(|index| lists[0].get(index).unwrap().clone())
                .collect(),
            types: (0..lists[1].len())
                .map(|index| lists[1].get(index).unwrap().clone())
                .collect(),
        });
        Ok(())
    })?;
    Ok(result)
}

fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn required_indexes_align_duplicate_scalars_by_logical_root_in_both_directions() {
    for code in [5, 6] {
        for descending in [false, true] {
            for forwarded in [false, true] {
                let bytes = fixture::source(code, descending, forwarded);
                let indexed = read_table_index_with_references(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    "IdOrder",
                    IndexReadRequest {
                        columns: Some(&["Id"]),
                        selection: IndexProjectionSelection::All,
                        maximum_rows: 4,
                    },
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let slots = if descending {
                    [0, 3, 1, 2]
                } else {
                    [1, 2, 3, 0]
                };
                assert_eq!(
                    indexed
                        .row_references()
                        .iter()
                        .map(|reference| (reference.page_number(), reference.slot_number()))
                        .collect::<Vec<_>>(),
                    slots.map(|slot| (12, slot))
                );
                let links = read_complex_column_links(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    "Score",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    links
                        .parents()
                        .iter()
                        .map(|parent| (
                            parent.row_reference().page_number(),
                            parent.row_reference().slot_number()
                        ))
                        .collect::<Vec<_>>(),
                    [(12, 0), (12, 1), (12, 2), (12, 3)]
                );
                assert_eq!(
                    links
                        .parents()
                        .iter()
                        .map(|parent| (parent.key(), parent.child_count()))
                        .collect::<Vec<_>>(),
                    [(Some(10), 2), (None, 0), (Some(20), 1), (Some(30), 0)]
                );
                let source = NativeSource::new(Cursor::new(bytes), SCOPE, budget(), &|| false)
                    .unwrap()
                    .with_collections()
                    .with_index("Items", "IdOrder")
                    .unwrap();
                let callbacks = Cell::new(0);
                assert_eq!(visit(&source, &callbacks).unwrap(), expected(descending));
                assert_eq!(callbacks.get(), 4);
                assert_eq!(
                    execute(
                        &source,
                        "SELECT Id,Count(Score.FileName) FROM Items GROUP BY Id ORDER BY Id"
                    )
                    .unwrap(),
                    [
                        vec![Value::Int32(10), Value::Int64(1)],
                        vec![Value::Int32(20), Value::Int64(0)],
                        vec![Value::Int32(30), Value::Int64(1)]
                    ]
                );
            }
        }
    }
}

struct Watch {
    bytes: Cursor<Vec<u8>>,
    blocked: Rc<Cell<Option<usize>>>,
    child_reads: Rc<Cell<usize>>,
    payload_reads: Rc<Cell<usize>>,
}

impl NativeReadSource for Watch {}

impl Read for Watch {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let page = self.bytes.position() as usize / fixture::PAGE;
        if page == 22 {
            self.child_reads.set(self.child_reads.get() + 1);
        }
        if page == PAYLOAD {
            self.payload_reads.set(self.payload_reads.get() + 1);
        }
        if self.blocked.get() == Some(page) {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        self.bytes.read(buffer)
    }
}

impl Seek for Watch {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn sequential_member_and_indexed_scalar_cache_warming_do_not_change_alignment() {
    let blocked = Rc::new(Cell::new(None));
    let child_reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watch {
            bytes: Cursor::new(fixture::source(6, false, true)),
            blocked: blocked.clone(),
            child_reads: child_reads.clone(),
            payload_reads: Rc::new(Cell::new(0)),
        },
        SCOPE,
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    let sequential = visit(&source, &Cell::new(0)).unwrap();
    assert_eq!(
        sequential
            .iter()
            .map(|parent| parent.id)
            .collect::<Vec<_>>(),
        [30, 10, 10, 20]
    );
    let cached_reads = child_reads.get();
    let source = source.with_index("Items", "IdOrder").unwrap();
    blocked.set(Some(22));
    assert_eq!(visit(&source, &Cell::new(0)).unwrap(), expected(false));
    assert_eq!(child_reads.get(), cached_reads);
    blocked.set(None);
    let mut scalars = Vec::new();
    source
        .scan_projected("Items", &[0], None, &mut |row| {
            scalars.push(row.get(0).unwrap().clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(
        scalars,
        [
            Value::Int32(10),
            Value::Int32(10),
            Value::Int32(20),
            Value::Int32(30)
        ]
    );
    blocked.set(Some(fixture::INDEX_ROOT));
    let callbacks = Cell::new(0);
    assert!(visit(&source, &callbacks).is_err());
    assert_eq!(callbacks.get(), 0);
    blocked.set(None);
    assert_eq!(visit(&source, &Cell::new(0)).unwrap(), expected(false));
}

#[test]
fn missing_duplicate_hidden_and_foreign_index_roots_fail_before_visitors() {
    let original = fixture::source(6, false, true);
    for entries in [
        vec![(10, 3073), (20, 3075), (30, 3072)],
        vec![(10, 3073), (10, 3073), (20, 3075), (30, 3072)],
        vec![(10, 3073), (10, 6656), (20, 3075), (30, 3072)],
        vec![(10, 3073), (10, 999 << 8), (20, 3075), (30, 3072)],
    ] {
        let mut bytes = original.clone();
        fixture::replace_index(&mut bytes, &entries);
        let source = NativeSource::new(Cursor::new(bytes), SCOPE, budget(), &|| false)
            .unwrap()
            .with_collections()
            .with_index("Items", "IdOrder")
            .unwrap();
        let callbacks = Cell::new(0);
        assert!(visit(&source, &callbacks).is_err());
        assert_eq!(callbacks.get(), 0);
    }
    let source = NativeSource::new(Cursor::new(original), SCOPE, budget(), &|| false)
        .unwrap()
        .with_collections()
        .with_index("Items", "Missing")
        .unwrap();
    let callbacks = Cell::new(0);
    assert!(visit(&source, &callbacks).is_err());
    assert_eq!(callbacks.get(), 0);
}

#[test]
fn late_orphan_and_required_index_io_failures_do_not_publish_partial_alignment() {
    let mut bytes = fixture::source(6, false, false);
    let (start, _) = modern_fixture::span(&bytes, 2);
    bytes[start + 14..start + 18].copy_from_slice(&999_i32.to_le_bytes());
    let source = NativeSource::new(Cursor::new(bytes), SCOPE, budget(), &|| false)
        .unwrap()
        .with_collections()
        .with_index("Items", "IdOrder")
        .unwrap();
    let callbacks = Cell::new(0);
    assert!(visit(&source, &callbacks).is_err());
    assert_eq!(callbacks.get(), 0);
    let blocked = Rc::new(Cell::new(Some(fixture::INDEX_ROOT)));
    let source = NativeSource::new(
        Watch {
            bytes: Cursor::new(fixture::source(6, false, false)),
            blocked: blocked.clone(),
            child_reads: Rc::new(Cell::new(0)),
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
    assert!(visit(&source, &callbacks).is_err());
    assert_eq!(callbacks.get(), 0);
    blocked.set(None);
    assert_eq!(visit(&source, &Cell::new(0)).unwrap(), expected(false));
}

#[test]
fn indexed_metadata_does_not_load_file_data_or_turn_failed_payloads_into_cached_values() {
    let mut descriptor = (0x40000000 | old_attachment::PAYLOAD.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend((PAYLOAD as u32 * 256).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut bytes = fixture::with_data(6, false, true, Some(&descriptor));
    bytes.resize((PAYLOAD + 1) * fixture::PAGE, 0);
    let mut page = support::data_page(0, &[(0, old_attachment::PAYLOAD.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    bytes[PAYLOAD * fixture::PAGE..(PAYLOAD + 1) * fixture::PAGE].copy_from_slice(&page);
    let blocked = Rc::new(Cell::new(Some(PAYLOAD)));
    let payload_reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watch {
            bytes: Cursor::new(bytes),
            blocked: blocked.clone(),
            child_reads: Rc::new(Cell::new(0)),
            payload_reads: payload_reads.clone(),
        },
        SCOPE,
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections()
    .with_index("Items", "IdOrder")
    .unwrap();
    assert_eq!(visit(&source, &Cell::new(0)).unwrap(), expected(false));
    assert_eq!(payload_reads.get(), 0);
    assert!(execute(&source, "SELECT Id,Score.FileData FROM Items").is_err());
    assert!(payload_reads.get() > 0);
    blocked.set(None);
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileData) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
    assert_eq!(visit(&source, &Cell::new(0)).unwrap(), expected(false));
}

#[test]
fn required_indexes_align_legacy_scalar_value_lists_with_duplicate_parent_values() {
    for descending in [false, true] {
        let source = NativeSource::new(
            Cursor::new(fixture::multivalue(6, descending, true)),
            SCOPE,
            budget(),
            &|| false,
        )
        .unwrap()
        .with_multivalues()
        .with_index("Items", "IdOrder")
        .unwrap();
        let mut observed = Vec::new();
        source
            .scan_multivalues("Items", &[0], &[0], &mut |row, lists| {
                assert_eq!(lists.len(), 1);
                observed.push((
                    row.get(0).unwrap().clone(),
                    (0..lists[0].len())
                        .map(|index| lists[0].get(index).unwrap().clone())
                        .collect::<Vec<_>>(),
                ));
                Ok(())
            })
            .unwrap();
        let absent = (Value::Int32(10), vec![]);
        let present = (Value::Int32(10), vec![Value::Int32(0)]);
        let empty = (Value::Int32(20), vec![]);
        let populated = (Value::Int32(30), vec![Value::Int32(100), Value::Null]);
        let expected = if descending {
            vec![populated, empty, absent, present]
        } else {
            vec![absent, present, empty, populated]
        };
        assert_eq!(observed, expected);
    }
}
