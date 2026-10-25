//! Independent grouped and progressive consumers of prior projection values.
use std::cell::Cell;
use std::cmp::Ordering;
use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringComparisonProfile},
    functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, StreamError, prepare,
    source::{CollationResolver, ProjectedRow, Source, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "projection_alias_execution/grouped.rs"]
mod grouped;
#[path = "projection_alias_execution/progressive.rs"]
mod progressive;

struct Records {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    visits: Cell<usize>,
    scans: Cell<usize>,
    swallow: bool,
    fail_eof: bool,
}

impl Records {
    fn new() -> Self {
        Self {
            columns: [
                ("N", ScalarType::Int32),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
            ]
            .into_iter()
            .map(|(name, kind)| Column::new(name.into(), kind, false).unwrap())
            .collect(),
            rows: [(1, "a"), (1, "A"), (2, "b")]
                .into_iter()
                .map(|(number, text)| {
                    vec![
                        Value::Int32(number),
                        Value::Text(text.into()),
                        Value::Text(text.into()),
                    ]
                })
                .collect(),
            policies: vec![
                None,
                Some(TextCollationId::from_host(1)),
                Some(TextCollationId::from_host(2)),
            ],
            visits: Cell::new(0),
            scans: Cell::new(0),
            swallow: false,
            fail_eof: false,
        }
    }
}

impl Source for Records {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&self.policies)
    }

    fn scan(
        &self,
        _table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }

    fn scan_projected_progressive(
        &self,
        _table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            self.visits.set(self.visits.get() + 1);
            let result = visitor(ProjectedRow::select(row, indices)?);
            if !self.swallow {
                result?;
            }
        }
        if self.fail_eof {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}

struct Units;

impl Collation for Units {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left
            .bytes()
            .map(|byte| byte.to_ascii_lowercase())
            .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase())))
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        let fold = |unit: &u16| {
            if (65..=90).contains(unit) {
                unit + 32
            } else {
                *unit
            }
        };
        Ok(left.iter().map(fold).cmp(right.iter().map(fold)))
    }
}

struct Resolver;

impl CollationResolver for Resolver {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert_eq!(policy.host_key(), 1);
        Ok(&Units)
    }
}

fn run(
    sql: &str,
    source: &Records,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), source)?
        .execute(source, &[], options)
        .map(|result| result.rows().to_vec())
}
