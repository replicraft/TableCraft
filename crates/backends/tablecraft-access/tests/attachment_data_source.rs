#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachments/fixture.rs"]
mod attachment_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "attachment_data_source/limits.rs"]
mod limits;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;
#[path = "attachment_data_source/metadata.rs"]
mod metadata;
#[path = "attachment_data_source/raw.rs"]
mod raw;
use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access as support;

// Independently written uncompressed wrapper for a zero-byte file, empty type.
const EMPTY_FILE: &[u8] = &[
    0, 0, 0, 0, 14, 0, 0, 0, 14, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0,
];
const PAGE_BYTES: usize = 4096;
const PARENT_PAGE: usize = 12;
const CHILD_PAGE: usize = 22;

struct PayloadWatch {
    page: usize,
    forbidden: Cell<bool>,
    reads: Cell<usize>,
}

struct Watching {
    bytes: Cursor<Vec<u8>>,
    forbid_records: Rc<Cell<bool>>,
    forbid_children: Rc<Cell<bool>>,
    reads: Rc<Cell<usize>>,
    payload: Option<Rc<PayloadWatch>>,
}
impl tablecraft_ace::NativeReadSource for Watching {}
impl Read for Watching {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let page = self.bytes.position() as usize / PAGE_BYTES;
        if let Some(payload) = &self.payload
            && page == payload.page
        {
            payload.reads.set(payload.reads.get() + 1);
            if payload.forbidden.get() {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
        }
        if matches!(page, PARENT_PAGE | CHILD_PAGE) {
            assert!(!self.forbid_records.get(), "binding read table records");
        }
        if page == CHILD_PAGE {
            assert!(!self.forbid_children.get(), "reread cached attachment rows");
            self.reads.set(self.reads.get() + 1);
        }
        self.bytes.read(bytes)
    }
}
impl Seek for Watching {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}
fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn decoded_payloads_align_preserve_null_empty_and_reuse_complete_cache() {
    for code in [2, 5, 6] {
        let mut bytes = attachment_fixture::with_payloads(attachment_fixture::PAYLOAD, EMPTY_FILE);
        bytes[20] = code;
        let forbid_records = Rc::new(Cell::new(true));
        let forbid_children = Rc::new(Cell::new(false));
        let reads = Rc::new(Cell::new(0));
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(bytes),
                forbid_records: forbid_records.clone(),
                forbid_children: forbid_children.clone(),
                reads: reads.clone(),
                payload: None,
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_decoded_attachments();
        let members = source.collection_members("Items").unwrap();
        assert_eq!(members.len(), 6);
        assert_eq!(
            members
                .iter()
                .find(|member| member.column().name() == "FileData")
                .unwrap()
                .column()
                .data_type(),
            ScalarType::Binary
        );
        let parsed =
            parse_query("SELECT Score.FileData FROM Items", ParseLimits::default()).unwrap();
        prepare(parsed.query(), &source).unwrap();
        assert_eq!(reads.get(), 0);
        forbid_records.set(false);
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Score.FileData AS Data,Score.FileType AS Kind FROM Items"
            )
            .unwrap(),
            [
                vec![
                    Value::Int32(1),
                    Value::Binary(vec![0, 1, 2, 255]),
                    Value::Text("field/type".into())
                ],
                vec![Value::Int32(1), Value::Null, Value::Null],
                vec![
                    Value::Int32(2),
                    Value::Binary(vec![]),
                    Value::Text("".into())
                ],
                vec![Value::Int32(3), Value::Null, Value::Null],
            ]
        );
        assert!(reads.get() > 0);
        forbid_children.set(true);
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Count(Score.FileData) AS N FROM Items GROUP BY Id ORDER BY Id"
            )
            .unwrap(),
            [
                vec![Value::Int32(1), Value::Int64(1)],
                vec![Value::Int32(2), Value::Int64(1)],
                vec![Value::Int32(3), Value::Int64(0)],
            ]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Score.FileData AS D FROM Items WHERE Score.FileData IS NOT NULL"
            )
            .unwrap(),
            [
                vec![Value::Int32(1), Value::Binary(vec![0, 1, 2, 255])],
                vec![Value::Int32(2), Value::Binary(vec![])],
            ]
        );
        assert_eq!(
            execute(&source, "SELECT Count(*) FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Score.FileData AS D FROM Items WHERE Score.FileFlags<0"
            )
            .unwrap(),
            [
                vec![Value::Int32(1), Value::Binary(vec![0, 1, 2, 255])],
                vec![Value::Int32(2), Value::Binary(vec![])]
            ]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Count(Score.FileName) FROM Items WHERE Score.FileData IS NOT NULL"
            )
            .unwrap(),
            [vec![Value::Int64(2)]]
        );
        assert_eq!(execute(&source, "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE A.Score.FileFlags<0 AND B.Score.FileData IS NULL ORDER BY A.Id").unwrap(), [vec![Value::Int32(1),Value::Int32(1)]]);
        assert_eq!(execute(&source, "SELECT A.Id,A.Score.FileData,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE B.Score.FileData IS NULL ORDER BY A.Id").unwrap(), [vec![Value::Int32(1),Value::Binary(vec![0,1,2,255]),Value::Int32(1)],vec![Value::Int32(1),Value::Null,Value::Int32(1)]]);
        let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Score.FileData IS NULL GROUP BY Id PIVOT Id";
        assert_eq!(
            execute(&source, sql).unwrap(),
            [vec![
                Value::Int32(1),
                Value::Int64(1),
                Value::Null,
                Value::Null
            ]]
        );
        let sql = "TRANSFORM Count(A.Score.FileData) SELECT A.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE B.Score.FileData IS NULL GROUP BY A.Id PIVOT A.Id IN (1,2,3)";
        assert_eq!(
            execute(&source, sql).unwrap(),
            [vec![
                Value::Int32(1),
                Value::Int64(1),
                Value::Null,
                Value::Null
            ]]
        );
        let sql = "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Score.FileData IS NULL)";
        assert_eq!(execute(&source, sql).unwrap(), [vec![Value::Int32(1)]]);
        let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE EXISTS(SELECT 1 WHERE A.Score.FileData IS NULL)";
        assert_eq!(
            execute(&source, sql).unwrap(),
            [vec![Value::Int32(1), Value::Int32(1)]]
        );
        let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id AND A.Score.FileData IS NOT NULL ORDER BY A.Id";
        assert_eq!(
            execute(&source, sql).unwrap(),
            [
                vec![Value::Int32(1), Value::Int32(1)],
                vec![Value::Int32(2), Value::Int32(2)]
            ]
        );
        let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id AND A.Score.FileData IS NOT NULL WHERE B.Score.FileData IS NULL";
        assert_eq!(
            execute(&source, sql).unwrap(),
            [vec![Value::Int32(1), Value::Int32(1)]]
        );
        let sql = "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Score.FileData IS NULL AND Score.FileFlags<0)";
        assert!(execute(&source, sql).unwrap().is_empty());
        let sql = "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Score.FileData IS NOT NULL AND Score.FileFlags<0) ORDER BY Id";
        assert_eq!(
            execute(&source, sql).unwrap(),
            [vec![Value::Int32(1)], vec![Value::Int32(2)]]
        );
        // Hidden negative criteria retain the explicit parent membership policy,
        // including empty parents; displayed FileData criteria above filter children.
        assert_eq!(
            execute(
                &source,
                "SELECT Id FROM Items WHERE Score.FileData IS NOT NULL ORDER BY Id"
            )
            .unwrap(),
            [vec![Value::Int32(2)], vec![Value::Int32(3)]]
        );
    }
}

#[test]
fn decoding_is_explicit_complete_and_not_hidden_by_filters_or_top() {
    let bytes = attachment_fixture::database(); // Last stored non-NULL wrapper is invalid.
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    assert_eq!(source.collection_members("Items").unwrap().len(), 6);
    assert!(execute(&source, "SELECT Score.FileData FROM Items").is_ok());
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_decoded_attachments();
    assert_eq!(source.collection_members("Items").unwrap().len(), 6);
    // Metadata/counts stay useful even on an invalid wrapper; no automatic decode.
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileName) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
    for sql in [
        "SELECT Score.FileData FROM Items",
        "SELECT TOP 1 Score.FileData FROM Items",
        "SELECT Score.FileData FROM Items WHERE Id=999",
        "SELECT Count(Score.FileData) FROM Items",
        "SELECT TOP 1 Score.FileName FROM Items WHERE Score.FileData IS NOT NULL",
        "SELECT TOP 1 A.Id FROM Items AS A,Items AS B WHERE A.Score.FileData IS NULL",
    ] {
        assert_eq!(
            execute(&source, sql).err(),
            Some(QueryError::Source),
            "{sql}"
        );
    }
    assert_eq!(
        execute(&source, "SELECT Count(*) FROM Items").unwrap(),
        [vec![Value::Int64(3)]]
    );
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileName) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
}

#[test]
fn selected_indexes_budgets_and_execution_cancellation_keep_their_boundaries() {
    let bytes = attachment_fixture::with_payloads(attachment_fixture::PAYLOAD, EMPTY_FILE);
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_decoded_attachments()
    .with_index("Items", "Missing")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Score.FileData FROM Items").err(),
        Some(QueryError::Unsupported)
    );
    // Find a bounded source that opens but cannot materialize the collection;
    // do not pin internal metadata allocation totals in the integration contract.
    let mut exhausted = false;
    for ceiling in [64 * 1024, 128 * 1024, 256 * 1024] {
        if let Ok(source) = NativeSource::new(
            Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            ResourceBudget::new(ceiling, 16384, 1_000_000),
            &|| false,
        ) {
            exhausted |= matches!(
                execute(
                    &source.with_decoded_attachments(),
                    "SELECT Score.FileData FROM Items"
                ),
                Err(QueryError::Limit(_))
            );
        }
    }
    assert!(exhausted);
    let calls = Cell::new(0);
    let armed = Cell::new(false);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        if armed.get() {
            calls.set(calls.get() + 1);
            calls.get() == stop.get()
        } else {
            false
        }
    };
    let run = || {
        armed.set(false);
        let source = NativeSource::new(
            Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            budget(),
            &cancelled,
        )
        .unwrap()
        .with_decoded_attachments();
        let parsed =
            parse_query("SELECT Score.FileData FROM Items", ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        calls.set(0);
        armed.set(true);
        plan.execute(&source, &[], ExecutionOptions::default())
            .map(|_| ())
    };
    run().unwrap();
    let total = calls.get();
    assert!(total > 10);
    for checkpoint in [1, total / 2, total] {
        stop.set(checkpoint);
        assert_eq!(run(), Err(QueryError::Cancelled));
    }
}

#[test]
fn signed_attachment_identities_keep_metadata_encoded_and_decoded_members() {
    const SLOT_DIRECTORY: usize = 14;
    const IDENTITY_OFFSET: usize = 6;

    for code in [2, 5, 6] {
        let mut bytes = attachment_fixture::with_payloads(attachment_fixture::PAYLOAD, EMPTY_FILE);
        bytes[20] = code;
        for (slot, identity) in [i32::MIN, -1, 0_i32].into_iter().enumerate() {
            let at = CHILD_PAGE * PAGE_BYTES + SLOT_DIRECTORY + slot * 2;
            let start = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize;
            support::put32(
                &mut bytes,
                CHILD_PAGE * PAGE_BYTES + start + IDENTITY_OFFSET,
                identity as u32,
            );
        }
        for decoded in [false, true] {
            let source = NativeSource::new(
                Cursor::new(bytes.clone()),
                CatalogScope::new(1),
                budget(),
                &|| false,
            )
            .unwrap();
            let source = if decoded {
                source.with_decoded_attachments()
            } else {
                source.with_collections()
            };
            let payload = if decoded {
                vec![0, 1, 2, 255]
            } else {
                attachment_fixture::PAYLOAD.to_vec()
            };
            let expected = vec![
                vec![Value::Binary(payload)],
                vec![Value::Null],
                vec![Value::Binary(if decoded {
                    vec![]
                } else {
                    EMPTY_FILE.to_vec()
                })],
                vec![Value::Null],
            ];
            assert_eq!(
                execute(&source, "SELECT Score.FileData FROM Items").unwrap(),
                expected
            );
            assert_eq!(
                execute(&source, "SELECT Id FROM Items WHERE Score IS NOT NULL").unwrap(),
                vec![vec![Value::Int32(1)], vec![Value::Int32(2)]]
            );
            assert_eq!(
                execute(&source, "SELECT Id FROM Items WHERE Score IS NULL").unwrap(),
                vec![vec![Value::Int32(3)]]
            );
            assert_eq!(
                execute(&source, "SELECT Score.FileData FROM Items").unwrap(),
                expected
            );
        }
    }
}
