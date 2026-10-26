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

#[path = "derived_multivalue_members/boundaries.rs"]
mod boundaries;
#[path = "multivalue_join_results/fixture.rs"]
mod join_fixture;
#[path = "derived_multivalue_members/values.rs"]
mod values;

use std::{
    cell::{Cell, RefCell},
    io::Cursor,
};
use tablecraft_access::{
    ExternalNativeSource, LinkedNativeSource, NativeExternalMapping, NativeLinkMapping,
    NativeSource,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, QueryValue, Source, prepare,
    prepare_with_options,
};
use tablecraft_sql::parse_query;

const PAGE_BYTES: usize = 4096;
const CHILD_DATA_PAGE: usize = 22;

fn allowance() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 32_768, 2_000_000)
}

fn members(id: i32, nullable: bool, offset: i32) -> Vec<Value> {
    match id {
        1 => vec![
            Value::Int32(101 + offset),
            Value::Int32(100 + offset),
            Value::Int32(101 + offset),
            Value::Null,
        ],
        2 if nullable => vec![Value::Null],
        2 => vec![Value::Int32(200 + offset)],
        3 => vec![],
        _ => panic!("unknown authored parent"),
    }
}

fn scalar(value: &QueryValue) -> &Value {
    let QueryValue::Scalar(ExpressionValue::Value(value)) = value else {
        panic!("expected exact scalar output");
    };
    value
}

fn list(value: &QueryValue) -> &[Value] {
    let QueryValue::Multivalue(value) = value else {
        panic!("expected a complete present list");
    };
    value.values()
}

fn execute(source: &dyn Source, sql: &str, declared: bool) -> Vec<Vec<QueryValue>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare_with_options(
        syntax.query(),
        source,
        PreparationOptions {
            declared_arithmetic: declared,
            ..Default::default()
        },
    )
    .unwrap();
    plan.execute_collections(
        source,
        &[],
        ExecutionOptions {
            declared_integral: declared,
            ..Default::default()
        },
    )
    .unwrap()
    .rows()
    .to_vec()
}

struct Named<'a> {
    source: &'a dyn Source,
    definition: tablecraft_sql::query::Query,
}

impl Source for Named<'_> {
    fn query(&self, name: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
        Ok((name == "Retained").then_some(&self.definition))
    }

    fn columns(&self, table: &str) -> Result<&[tablecraft_schema::Column], QueryError> {
        self.source.columns(table)
    }

    fn text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<tablecraft_query::source::TextCollationId>], QueryError> {
        self.source.text_collations(table)
    }

    fn unavailable_columns(
        &self,
        table: &str,
    ) -> Result<&[tablecraft_schema::SchemaName], QueryError> {
        self.source.unavailable_columns(table)
    }

    fn multivalue_columns(&self, table: &str) -> Result<&[tablecraft_schema::Column], QueryError> {
        self.source.multivalue_columns(table)
    }

    fn multivalue_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<tablecraft_query::source::TextCollationId>], QueryError> {
        self.source.multivalue_text_collations(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.source.scan(table, visitor)
    }

    fn scan_multivalues(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        visitor: &mut tablecraft_query::source::MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.source
            .scan_multivalues(table, indices, values, visitor)
    }
}
