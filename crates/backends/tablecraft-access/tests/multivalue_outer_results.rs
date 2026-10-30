use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;
use tablecraft_testkit::access as support;

#[path = "multivalue_outer_results/boundaries.rs"]
mod boundaries;
#[path = "multivalue_outer_results/forwarding.rs"]
mod forwarding;
#[path = "multivalue_join_results/fixture.rs"]
mod join_fixture;
#[path = "multivalue_outer_results/values.rs"]
mod values;

use join_fixture::database as fixture;
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
    limits::{Resource, ResourceBudget},
    values::{ExpressionValue, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, OutputType, QueryError, QueryValue, Source, prepare};
use tablecraft_sql::parse_query;

const PAGE_BYTES: usize = 4096;
const CHILD_DATA_PAGE: usize = 22;
const PARENT_DATA_PAGE: usize = 12;

fn allowance() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 32_768, 2_000_000)
}

fn members(id: i32, offset: i32) -> Vec<Value> {
    match id {
        1 => vec![
            Value::Int32(101 + offset),
            Value::Int32(100 + offset),
            Value::Int32(101 + offset),
            Value::Null,
        ],
        2 => vec![Value::Int32(200 + offset)],
        3 => vec![],
        _ => panic!("Unknown independently authored parent identifier"),
    }
}

fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<QueryValue>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute_collections(source, &[], Default::default())?
        .rows()
        .to_vec())
}

fn scalar(value: &QueryValue) -> Value {
    let QueryValue::Scalar(ExpressionValue::Value(value)) = value else {
        panic!("Expected exact stored scalar output");
    };
    value.clone()
}

fn list(value: &QueryValue) -> Vec<Value> {
    let QueryValue::Multivalue(value) = value else {
        panic!("Expected typed list output, including for an empty parent");
    };
    value.values().to_vec()
}

fn absent(value: &QueryValue) {
    assert!(matches!(value, QueryValue::AbsentMultivalue));
}

fn expected(id: Option<i32>) -> Option<Vec<Value>> {
    id.map(|id| match id {
        1 => members(1, 0),
        2 => vec![Value::Null],
        3 => vec![],
        _ => panic!("Unknown fixture parent"),
    })
}

fn assert_outer(row: &[QueryValue], left: Option<i32>, right: Option<i32>) {
    assert_eq!(row.len(), 4);
    assert_eq!(
        scalar(&row[0]),
        left.map(Value::Int32).unwrap_or(Value::Null)
    );
    assert_eq!(
        scalar(&row[1]),
        right.map(Value::Int32).unwrap_or(Value::Null)
    );
    for (value, wanted) in [(&row[2], expected(left)), (&row[3], expected(right))] {
        if let Some(wanted) = wanted {
            assert_eq!(list(value), wanted);
        } else {
            absent(value);
        }
    }
}

struct Watching<'a> {
    bytes: Cursor<Vec<u8>>,
    forbid_rows: &'a Cell<bool>,
    forbid_children: &'a Cell<bool>,
    child_reads: &'a Cell<usize>,
}

impl NativeReadSource for Watching<'_> {}

impl Read for Watching<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let page = self.bytes.position() as usize / PAGE_BYTES;
        if page == CHILD_DATA_PAGE || page == PARENT_DATA_PAGE {
            assert!(
                !self.forbid_rows.get(),
                "Metadata/unsupported query read native records"
            );
        }
        if page == CHILD_DATA_PAGE {
            assert!(
                !self.forbid_children.get(),
                "Scalar-only leaf requested collection records"
            );
            self.child_reads.set(self.child_reads.get() + 1);
        }
        self.bytes.read(bytes)
    }
}

impl Seek for Watching<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}
