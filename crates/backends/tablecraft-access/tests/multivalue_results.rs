#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

#[path = "multivalue_results/boundaries.rs"]
mod boundaries;
#[path = "multivalue_results/forwarding.rs"]
mod forwarding;
#[path = "multivalue_results/values.rs"]
mod values;

use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_access::{
    ExternalNativeSource, LinkedNativeSource, NativeExternalMapping, NativeLinkMapping,
    NativeSource,
};
use tablecraft_ace::NativeReadSource;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, QueryValue, Source, prepare};
use tablecraft_sql::parse_query;

const PAGE_BYTES: usize = 4096;
const HEADER_VERSION: usize = 20;
const CHILD_DEFINITION: usize = 20;
const CHILD_PAGE: usize = 22;
const PARENT_PAGE: usize = 12;
const DEFINITION_ROW_COUNT: usize = 16;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000)
}

fn populated(code: u8) -> Vec<u8> {
    let mut bytes = links_fixture::populated();
    bytes[HEADER_VERSION] = code;
    bytes
}

fn custom_members(code: u8, members: &[(i32, Option<i32>, i32)]) -> Vec<u8> {
    let mut bytes = populated(code);
    support::put32(
        &mut bytes,
        CHILD_DEFINITION * PAGE_BYTES + DEFINITION_ROW_COUNT,
        members.len() as u32,
    );
    let rows = members
        .iter()
        .map(|(key, value, identity)| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend(key.to_le_bytes());
            row.extend(value.unwrap_or_default().to_le_bytes());
            row.extend(identity.to_le_bytes());
            row.push(if value.is_some() { 7 } else { 5 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(CHILD_DEFINITION as u32, &rows));
    bytes
}

fn query(source: &dyn Source, sql: &str) -> Result<Vec<Vec<QueryValue>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    let prepared = prepare(parsed.query(), source)?;
    Ok(prepared
        .execute_collections(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

fn observe(rows: &[Vec<QueryValue>]) -> Vec<(i32, Vec<Value>)> {
    rows.iter()
        .map(|row| {
            assert_eq!(row.len(), 2);
            let QueryValue::Scalar(ExpressionValue::Value(Value::Int32(id))) = &row[0] else {
                panic!("Expected exact native Int32 parent identifier");
            };
            let QueryValue::Multivalue(values) = &row[1] else {
                panic!("Expected a typed collection, including for an empty parent");
            };
            (*id, values.values().to_vec())
        })
        .collect()
}

struct Watching<'a> {
    bytes: Cursor<Vec<u8>>,
    forbid_rows: &'a Cell<bool>,
    fail_children: &'a Cell<bool>,
    child_reads: &'a Cell<usize>,
}

impl NativeReadSource for Watching<'_> {}

impl Read for Watching<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let page = self.bytes.position() as usize / PAGE_BYTES;
        if page == PARENT_PAGE || page == CHILD_PAGE {
            assert!(
                !self.forbid_rows.get(),
                "Preparation/guard read table records"
            );
        }
        if page == CHILD_PAGE {
            self.child_reads.set(self.child_reads.get() + 1);
            if self.fail_children.get() {
                return Err(std::io::Error::other("Injected child read failure"));
            }
        }
        self.bytes.read(bytes)
    }
}

impl Seek for Watching<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}
