#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/saved_queries/fixture.rs"]
mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

const USER_DATA_PAGE: u64 = 12;
struct Counting {
    bytes: Cursor<Vec<u8>>,
    reads: Rc<Cell<usize>>,
    user_reads: Rc<Cell<usize>>,
}
impl tablecraft_ace::NativeReadSource for Counting {}
impl Read for Counting {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        if self.bytes.position() / fixture::PAGE_BYTES as u64 == USER_DATA_PAGE {
            self.user_reads.set(self.user_reads.get() + 1);
        }
        self.bytes.read(bytes)
    }
}
impl Seek for Counting {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}
fn database(cycle: bool) -> Vec<u8> {
    let row = |object, attribute| fixture::Fragment {
        object: Some(object),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    let fragments = [
        fixture::Fragment {
            name1: Some("Empty"),
            ..row(fixture::QUERY_ID, 5)
        },
        fixture::Fragment {
            expression: Some("[Id]"),
            ..row(fixture::QUERY_ID, 6)
        },
        fixture::Fragment {
            name1: Some(if cycle { "Saved" } else { "Items" }),
            ..row(fixture::EMPTY_ID, 5)
        },
        fixture::Fragment {
            expression: Some("[Id]"),
            ..row(fixture::EMPTY_ID, 6)
        },
    ];
    fixture::database_with_query_flags(&fragments, 0x80000000)
}
#[test]
fn native_named_definitions_bind_without_user_rows_and_reuse_complete_metadata() {
    let reads = Rc::new(Cell::new(0));
    let user_reads = Rc::new(Cell::new(0));
    let mut bytes = database(false);
    bytes[20] = 2;
    let source = NativeSource::new(
        Counting {
            bytes: Cursor::new(bytes),
            reads: reads.clone(),
            user_reads: user_reads.clone(),
        },
        CatalogScope::new(9),
        ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
    .unwrap();
    let sql = parse_query(
        "SELECT Saved.Id FROM Saved ORDER BY Saved.Id",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(sql.query(), &source).unwrap();
    assert_eq!(user_reads.get(), 0);
    let metadata = reads.get();
    assert!(source.query("sAvEd").unwrap().is_some());
    assert!(source.query("EMPTY").unwrap().is_some());
    assert_eq!(reads.get(), metadata);
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    assert!(user_reads.get() > 0);
}
#[test]
fn native_cycles_and_table_query_name_collisions_fail_during_binding() {
    for ambiguous in [false, true] {
        let mut bytes = database(true);
        if ambiguous {
            let at = bytes
                .windows(5)
                .position(|value| value == b"Empty")
                .unwrap();
            bytes[at..at + 5].copy_from_slice(b"Items");
        }
        let source = NativeSource::new(
            Cursor::new(bytes),
            CatalogScope::new(9),
            ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
            &|| false,
        )
        .unwrap();
        let sql = parse_query(
            if ambiguous {
                "SELECT * FROM Items"
            } else {
                "SELECT * FROM Saved"
            },
            ParseLimits::default(),
        )
        .unwrap();
        assert_eq!(
            prepare(sql.query(), &source).err(),
            Some(if ambiguous {
                QueryError::AmbiguousName
            } else {
                QueryError::DependencyCycle
            })
        );
    }
}
