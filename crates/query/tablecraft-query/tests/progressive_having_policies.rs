//! Rejected groups do not merge or activate deferred projection String policies.
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, QueryError, StreamError, prepare,
    source::{CollationResolver, Source, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Input {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    active: Cell<bool>,
    eof: Cell<bool>,
}

impl Input {
    fn new() -> Self {
        Self {
            columns: [
                ("K", ScalarType::Int32),
                ("Id", ScalarType::Int32),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
            ]
            .map(|(name, kind)| Column::new(name.into(), kind, false).unwrap())
            .to_vec(),
            rows: [(0, 1), (0, 2), (1, 3), (1, 4)]
                .map(|(key, id)| {
                    vec![
                        Value::Int32(key),
                        Value::Int32(id),
                        Value::Text("alpha".into()),
                        Value::Text("beta".into()),
                    ]
                })
                .to_vec(),
            policies: vec![
                None,
                None,
                Some(TextCollationId::from_host(1)),
                Some(TextCollationId::from_host(2)),
            ],
            active: Cell::new(false),
            eof: Cell::new(false),
        }
    }
}

impl Source for Input {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        assert_eq!(table, "T");
        assert!(!self.active.get());
        Ok(&self.columns)
    }

    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        assert!(!self.active.get());
        Ok(&self.policies)
    }

    fn scan(
        &self,
        _table: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("HAVING policy controls must not materialize a source snapshot");
    }

    fn scan_projected_progressive(
        &self,
        _table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(!self.active.replace(true));
        self.eof.set(false);
        let result = (|| {
            for row in &self.rows {
                visitor(ProjectedRow::select(row, indices)?)?;
            }
            self.eof.set(true);
            Ok(())
        })();
        self.active.set(false);
        result
    }
}

struct Order;

impl Collation for Order {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

static ORDINAL: Order = Order;

struct Resolver<'a> {
    source: &'a Input,
    calls: Cell<usize>,
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(self.source.eof.get());
        assert!(!self.source.active.get());
        assert_eq!(id.host_key(), 1);
        self.calls.set(self.calls.get() + 1);
        Ok(&ORDINAL)
    }
}

#[test]
fn rejected_actual_string_conflicts_stay_inert_but_accepted_conflicts_refuse_after_eof() {
    let source = Input::new();
    for declared_integral in [false, true] {
        for predicate in ["False", "NULL", "K=1", "K=0"] {
            let resolver = Resolver {
                source: &source,
                calls: Cell::new(0),
            };
            // Each concatenation discards latent policies of its actual numeric
            // operand. Only row 2 contributes B; every other row contributes A.
            // Thus group 0 contains actual A/B String lineage, while group 1
            // genuinely contains A lineage only (unlike IIf(Id=2,B,A)).
            let sql = format!(
                "SELECT K,Min(IIf(Id=2,B,Id)&IIf(Id<>2,A,0)) FROM T GROUP BY K HAVING {predicate}"
            );
            let syntax = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare(syntax.query(), &source).unwrap();
            let mut rows = Vec::new();
            let actual = plan.execute_progressive(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral,
                    collation_resolver: Some(&resolver),
                    number_format: Some(NumberFormat::Invariant),
                    ..Default::default()
                },
                &mut |row| {
                    assert!(source.eof.get());
                    rows.push(row.to_vec());
                    Ok::<_, ()>(())
                },
            );

            if predicate == "K=0" {
                assert!(matches!(
                    actual,
                    Err(StreamError::Query(QueryError::CollationConflict))
                ));
                assert!(rows.is_empty());
                assert_eq!(resolver.calls.get(), 0);
            } else {
                let summary = actual.unwrap();
                assert_eq!(summary.scanned_rows, 4);
                if predicate == "K=1" {
                    assert_eq!(rows, [vec![Value::Int32(1), Value::Text("3alpha".into())]]);
                    assert!(resolver.calls.get() > 0);
                } else {
                    assert!(rows.is_empty());
                    assert_eq!(resolver.calls.get(), 0);
                }
                assert_eq!(summary.emitted_rows, rows.len() as u64);
            }
            assert!(source.eof.get());
        }
    }
}
