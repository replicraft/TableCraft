#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/index_fixture.rs"]
mod index_fixture;
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

const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const USAGE_PAGE: u32 = 11;
const ROW_COUNT: u32 = 3;
fn database(bad: bool) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes[TABLE_PAGE * PAGE_BYTES..(TABLE_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &fixture::definition(
            &[("Id", 4, 0, 4), ("BadDate", 8, 4, 8), ("Score", 4, 12, 4)],
            ROW_COUNT,
            USAGE_PAGE << 8,
        ),
    );
    let rows = (1..=ROW_COUNT)
        .map(|id| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend((id as i32).to_le_bytes());
            row.extend(
                (if bad { f64::NAN } else { 0.0_f64 })
                    .to_bits()
                    .to_le_bytes(),
            );
            row.extend((id as i32 * 10).to_le_bytes());
            row.push(7);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(TABLE_PAGE as u32, &rows));
    bytes
}
struct Counting {
    data: Cursor<Vec<u8>>,
    reads: Rc<Cell<usize>>,
}
impl Read for Counting {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        self.data.read(bytes)
    }
}
impl Seek for Counting {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.data.seek(position)
    }
}
fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn unused_ordinary_payload_errors_are_skipped_and_complete_supersets_are_cached() {
    let reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Counting {
            data: Cursor::new(database(true)),
            reads: reads.clone(),
        },
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Id, Score FROM Items ORDER BY Id").unwrap(),
        [
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Int32(30)]
        ]
    );
    let loaded = reads.get();
    assert_eq!(
        execute(&source, "SELECT Score FROM Items WHERE Id>1 ORDER BY Id").unwrap(),
        [vec![Value::Int32(20)], vec![Value::Int32(30)]]
    );
    assert_eq!(
        execute(&source, "SELECT Score FROM Items ORDER BY Score").unwrap(),
        [
            vec![Value::Int32(10)],
            vec![Value::Int32(20)],
            vec![Value::Int32(30)]
        ]
    );
    assert_eq!(
        execute(&source, "SELECT Count(*) FROM Items").unwrap(),
        [vec![Value::Int64(3)]]
    );
    assert_eq!(reads.get(), loaded);
    assert!(execute(&source, "SELECT BadDate FROM Items").is_err());
    assert!(execute(&source, "SELECT * FROM Items").is_err());
    let failed = reads.get();
    assert_eq!(
        execute(&source, "SELECT Id FROM Items ORDER BY Id").unwrap(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    assert_eq!(reads.get(), failed);
}

#[test]
fn distinct_projection_shapes_and_correlated_reuse_do_not_reload_cached_payloads() {
    let reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Counting {
            data: Cursor::new(database(false)),
            reads: reads.clone(),
        },
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
    .unwrap();
    execute(&source, "SELECT Id FROM Items").unwrap();
    let first = reads.get();
    execute(&source, "SELECT Score FROM Items").unwrap();
    assert!(reads.get() > first);
    let both = reads.get();
    execute(&source, "SELECT Id FROM Items").unwrap();
    execute(&source, "SELECT Score FROM Items").unwrap();
    assert_eq!(
        execute(
            &source,
            "SELECT A.Id, (SELECT Count(*) FROM Items AS B WHERE B.Score>A.Id) FROM Items AS A",
        )
        .unwrap(),
        [
            vec![Value::Int32(1), Value::Int64(3)],
            vec![Value::Int32(2), Value::Int64(3)],
            vec![Value::Int32(3), Value::Int64(3)]
        ]
    );
    assert_eq!(reads.get(), both);
}

#[test]
fn indexed_projection_skips_bad_nonkey_payloads_but_validates_hidden_keys() {
    const COLUMN_START: usize = 75;
    const COLUMN_BYTES: usize = 25;
    const FLAGS_OFFSET: usize = 15;
    const FIXED_OFFSET: usize = 21;
    const LENGTH_OFFSET: usize = 23;
    let mut bytes = index_fixture::database(false, false, false);
    let definition = &mut bytes[TABLE_PAGE * PAGE_BYTES..(TABLE_PAGE + 1) * PAGE_BYTES];
    // Keep the Score index and native key ordinal; replace non-key Id with DATE.
    definition[COLUMN_START] = 8;
    definition[COLUMN_START + FLAGS_OFFSET] = 3;
    support::put16(definition, COLUMN_START + LENGTH_OFFSET, 8);
    support::put16(definition, COLUMN_START + COLUMN_BYTES + FIXED_OFFSET, 8);
    let rows = [10_i32, 20, 30]
        .into_iter()
        .map(|score| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(f64::NAN.to_bits().to_le_bytes());
            row.extend(score.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(TABLE_PAGE as u32, &rows));
    let source = NativeSource::new(
        Cursor::new(bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
    .unwrap()
    .with_index("Items", "ScoreKey")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Score FROM Items WHERE Score=20").unwrap(),
        [vec![Value::Int32(20)]]
    );
    assert_eq!(
        execute(&source, "SELECT Count(*) FROM Items").unwrap(),
        [vec![Value::Int64(3)]]
    );
    assert!(execute(&source, "SELECT Id FROM Items").is_err());
}
