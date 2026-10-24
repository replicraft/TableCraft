#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "version_history_source/compression.rs"]
mod compression;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/version_history/fixture.rs"]
mod fixture;
#[path = "version_history_source/mixed.rs"]
mod mixed;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
#[path = "version_history_source/visible_memo.rs"]
mod visible_memo;

use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{LegacyDateTime, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

const PAGE_BYTES: usize = 4096;
const PARENT_PAGE: usize = 12;
const CHILD_PAGE: usize = 22;

struct Watching {
    bytes: Cursor<Vec<u8>>,
    forbid_records: Rc<Cell<bool>>,
    forbid_children: Rc<Cell<bool>>,
    child_reads: Rc<Cell<usize>>,
}

impl tablecraft_ace::NativeReadSource for Watching {}

impl Read for Watching {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let page = self.bytes.position() as usize / PAGE_BYTES;
        if matches!(page, PARENT_PAGE | CHILD_PAGE) {
            assert!(
                !self.forbid_records.get(),
                "binding read stored history records"
            );
        }
        if page == CHILD_PAGE {
            assert!(!self.forbid_children.get(), "reread complete history cache");
            self.child_reads.set(self.child_reads.get() + 1);
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

fn date(bits: u64) -> Value {
    Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
}

#[test]
fn stored_history_members_bind_without_records_and_keep_alignment_order_and_cache() {
    for code in [2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[20] = code;
        let forbid_records = Rc::new(Cell::new(true));
        let forbid_children = Rc::new(Cell::new(false));
        let child_reads = Rc::new(Cell::new(0));
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(bytes.clone()),
                forbid_records: forbid_records.clone(),
                forbid_children: forbid_children.clone(),
                child_reads: child_reads.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_collections();
        let members = source.collection_members("Items").unwrap();
        assert_eq!(members.len(), 2);
        assert!(
            members
                .iter()
                .all(|member| member.collection().as_str() == "Score")
        );
        assert_eq!(members[0].column().name(), "Notes");
        assert_eq!(members[1].column().name(), "Modified");
        let sql = "SELECT Id,Notes AS CurrentMemo,Score.Notes AS HistoryText,Score.Modified AS HistoryTime FROM Items";
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(child_reads.get(), 0);

        forbid_records.set(false);
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            [
                vec![
                    Value::Int32(1),
                    Value::Null,
                    Value::Text("é🦀\0older".into()),
                    date((-0.25_f64).to_bits())
                ],
                vec![
                    Value::Int32(1),
                    Value::Null,
                    Value::Text("newer".into()),
                    date(0.25_f64.to_bits())
                ],
                vec![
                    Value::Int32(2),
                    Value::Null,
                    Value::Text("".into()),
                    date(1)
                ],
                vec![Value::Int32(2), Value::Null, Value::Null, Value::Null],
                vec![Value::Int32(3), Value::Null, Value::Null, Value::Null],
            ]
        );
        assert!(child_reads.get() > 0);
        forbid_children.set(true);
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Count(Score.Notes) AS N FROM Items GROUP BY Id ORDER BY Id"
            )
            .unwrap(),
            [
                vec![Value::Int32(1), Value::Int64(2)],
                vec![Value::Int32(2), Value::Int64(1)],
                vec![Value::Int32(3), Value::Int64(0)]
            ]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Score.Notes,Score.Modified FROM Items WHERE Score.Modified IS NOT NULL ORDER BY Id"
            )
            .unwrap(),
            [
                vec![
                    Value::Int32(1),
                    Value::Text("é🦀\0older".into()),
                    date((-0.25_f64).to_bits())
                ],
                vec![
                    Value::Int32(1),
                    Value::Text("newer".into()),
                    date(0.25_f64.to_bits())
                ],
                vec![Value::Int32(2), Value::Text("".into()), date(1)],
            ]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Id FROM Items WHERE Score IS NOT NULL ORDER BY Id"
            )
            .unwrap(),
            [vec![Value::Int32(1)], vec![Value::Int32(2)]]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Id FROM Items WHERE Score IS NULL ORDER BY Id"
            )
            .unwrap(),
            [vec![Value::Int32(3)]]
        );
        assert_eq!(
            execute(&source, "SELECT Count(*) FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
        assert!(execute(&source, "SELECT Score.FileData FROM Items").is_err());
    }
}

#[test]
fn history_is_explicit_and_selected_indexes_never_silently_fall_back() {
    let bytes = fixture::database();
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    assert!(source.collection_members("Items").unwrap().is_empty());
    assert!(execute(&source, "SELECT Score.Notes FROM Items").is_err());
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    assert!(source.collection_members("Items").unwrap().is_empty());
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_collections()
    .with_index("Items", "Missing")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Score.Notes FROM Items").err(),
        Some(QueryError::Unsupported)
    );
}

#[test]
fn history_corruption_is_not_hidden_by_top_filters_or_counts() {
    let mut bytes = fixture::database();
    let page = CHILD_PAGE * PAGE_BYTES;
    // Corrupt the fourth/last child, after valid records that TOP could otherwise
    // publish. The complete selected history must validate before any result.
    let row_offset = u16::from_le_bytes(bytes[page + 20..page + 22].try_into().unwrap()) as usize;
    bytes[page + row_offset + 2..page + row_offset + 6].copy_from_slice(&999_i32.to_le_bytes());
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_collections();
    for sql in [
        "SELECT TOP 1 Score.Notes FROM Items",
        "SELECT Score.Modified FROM Items WHERE Id=999",
        "SELECT Count(Score.Notes) FROM Items",
        "SELECT Count(*) FROM Items WHERE Score IS NOT NULL",
    ] {
        assert!(execute(&source, sql).is_err(), "{sql}");
    }
    assert_eq!(
        execute(&source, "SELECT Count(*) FROM Items").unwrap(),
        [vec![Value::Int64(3)]]
    );
}

#[test]
fn histories_respect_cumulative_limits_and_execution_cancellation() {
    let bytes = fixture::database();
    let mut exhausted = false;
    for ceiling in [64 * 1024, 128 * 1024, 256 * 1024] {
        if let Ok(source) = NativeSource::new(
            Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            ResourceBudget::new(ceiling, 16384, 1_000_000),
            &|| false,
        ) {
            exhausted |= matches!(
                execute(&source.with_collections(), "SELECT Score.Notes FROM Items"),
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
        .with_collections();
        let parsed = parse_query("SELECT Score.Notes FROM Items", ParseLimits::default()).unwrap();
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
fn history_text_policy_comes_from_flat_member_not_conceptual_template() {
    for version in [0, 1] {
        let mut bytes = fixture::database_with(&[(10, 1, Some("A"), None)]);
        for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
            bytes[110 + index] = [9, 4, 0, 1][index] ^ mask;
        }
        // Independent native descriptor bytes on flat Notes. The conceptual
        // template deliberately has a different/unknown sort descriptor.
        let at = 20 * PAGE_BYTES + 63 + 25 + 11;
        bytes[at..at + 4].copy_from_slice(&[9, 4, 0, version]);
        let pool = RefCell::new(budget());
        let (source, resolver) =
            NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
                false
            })
            .unwrap()
            .with_collections()
            .with_database_collation_resolver(&pool)
            .unwrap();
        let policies = source.collection_member_text_collations("Items").unwrap();
        assert_eq!(
            policies[0].unwrap().host_key(),
            u32::from_le_bytes([9, 4, 0, version]) as u64
        );
        assert_eq!(policies[1], None);
        let parsed = parse_query(
            "SELECT Id FROM Items WHERE Score.Notes='a'",
            ParseLimits::default(),
        )
        .unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(resolver.default_collation()),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int32(1)]]);
    }
}

#[test]
fn unsupported_history_candidate_keeps_unrelated_members_and_scalars_available() {
    let original = mixed::database();
    let source = NativeSource::new(
        Cursor::new(original.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections();
    assert_eq!(source.collection_members("Items").unwrap().len(), 4);

    // Independently decline at visible-Memo qualification and flat-field
    // qualification. Neither unsupported shape is a corrupt native mapping.
    for at in [
        mixed::OWNER_PAGE * PAGE_BYTES + mixed::FIELD_START + 4 * mixed::FIELD_BYTES + 16,
        mixed::OTHER_FLAT_PAGE * PAGE_BYTES + mixed::FIELD_START + mixed::FIELD_BYTES + 16,
    ] {
        let mut bytes = original.clone();
        bytes[at] = 0x10;
        let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
            false
        })
        .unwrap()
        .with_collections();
        let members = source.collection_members("Items").unwrap();
        assert_eq!(members.len(), 2);
        assert!(
            members
                .iter()
                .all(|member| member.collection().as_str() == "Score")
        );
        assert!(
            source
                .unavailable_columns("Items")
                .unwrap()
                .iter()
                .any(|name| name.as_str() == "OtherHistory")
        );
        assert_eq!(
            execute(&source, "SELECT Count(Score.Notes) FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
        assert_eq!(
            execute(&source, "SELECT Count(*) FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
        for sql in [
            "SELECT OtherHistory.Details FROM Items",
            "SELECT OtherHistory.Modified FROM Items",
            "SELECT Id FROM Items WHERE OtherHistory IS NOT NULL",
        ] {
            assert!(execute(&source, sql).is_err(), "{sql}");
        }
    }
}
