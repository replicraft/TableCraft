//! Independent value/composition controls over original malformed-cache fixtures.
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

#[path = "recomputed_view_conformance/fixtures.rs"]
mod fixtures;
#[path = "recomputed_view_conformance/retained.rs"]
mod retained;
#[path = "recomputed_view_conformance/values.rs"]
mod values;

use std::{cmp::Ordering, io::Cursor};
use tablecraft_access::{CalculationView, NativeSource, RecomputedNativeSource};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Value},
};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, OutputType, QueryError, Source, prepare};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{ParseLimits, parse_query};

const PAGE_BYTES: usize = 4096;

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral: declared,
        collation: Some(&Ordinal),
        ..Default::default()
    }
}

fn native(bytes: Vec<u8>) -> NativeSource<'static, Cursor<Vec<u8>>> {
    NativeSource::new(
        Cursor::new(bytes),
        CatalogScope::new(41),
        ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000),
        &|| false,
    )
    .unwrap()
}

fn rows(source: &dyn Source, sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(syntax.query(), source)?;
    Ok(plan
        .execute(source, &[], options(declared))?
        .rows()
        .to_vec())
}

fn scalar_type(output: OutputType) -> Option<ScalarType> {
    match output {
        OutputType::Scalar { data_type, .. } => Some(data_type.scalar_type()),
        OutputType::Null | OutputType::Dynamic => None,
        OutputType::Multivalue { .. } => {
            panic!("A scalar calculation must not acquire collection output metadata")
        }
    }
}
