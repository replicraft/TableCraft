use crate::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{CollationResolver, MemorySource, MemoryTable, TextCollationId, TextEqualityKey},
};
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

const POLICY: TextCollationId = TextCollationId::from_host(7);
const OTHER_POLICY: TextCollationId = TextCollationId::from_host(8);

struct Rows(MemorySource);

impl Source for Rows {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.0.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        if table == "U" {
            Ok(&[Some(OTHER_POLICY)])
        } else {
            Ok(&[Some(POLICY)])
        }
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.0.scan(table, visitor)
    }
}

fn source(values: &[&str]) -> Rows {
    let table = |name: &str| {
        MemoryTable::new(
            name.into(),
            vec![Column::new("K".into(), ScalarType::Text, false).unwrap()],
            values
                .iter()
                .map(|value| vec![Value::Text((*value).into())])
                .collect(),
        )
        .unwrap()
    };
    Rows(MemorySource::new(vec![table("T"), table("U")]).unwrap())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Collide,
    Unsupported,
    Cancelled,
    WrongPolicy,
}

struct Resolver {
    calls: Cell<usize>,
    mode: Mode,
}

impl Collation for Resolver {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left
            .trim_end_matches(' ')
            .to_ascii_lowercase()
            .cmp(&right.trim_end_matches(' ').to_ascii_lowercase()))
    }
}

impl CollationResolver for Resolver {
    fn resolve(&self, _: TextCollationId) -> Result<&dyn Collation, QueryError> {
        Ok(self)
    }

    fn grouping_key(
        &self,
        policy: TextCollationId,
        text: &str,
    ) -> Result<Option<TextEqualityKey>, QueryError> {
        self.calls.set(self.calls.get() + 1);
        match self.mode {
            Mode::Unsupported if text == "b" => Ok(None),
            Mode::Cancelled => Err(QueryError::Cancelled),
            Mode::WrongPolicy => {
                TextEqualityKey::from_host(TextCollationId::from_host(99), b"collision").map(Some)
            }
            _ => TextEqualityKey::from_host(policy, b"collision").map(Some),
        }
    }
}

fn run(sql: &str, source: &Rows, resolver: &Resolver) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(resolver),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn collisions_compare_fully_and_unsupported_hint_keeps_every_prior_representative() {
    for mode in [Mode::Collide, Mode::Unsupported] {
        let resolver = Resolver {
            calls: Cell::new(0),
            mode,
        };
        let rows = run(
            "SELECT K,Count(*) FROM T GROUP BY K",
            &source(&["A", "b", "a", "B "]),
            &resolver,
        )
        .unwrap();
        assert_eq!(
            rows,
            vec![
                vec![Value::Text("A".into()), Value::Int64(2)],
                vec![Value::Text("b".into()), Value::Int64(2)]
            ]
        );
        assert_eq!(
            resolver.calls.get(),
            if mode == Mode::Collide { 4 } else { 2 }
        );
    }
}

#[test]
fn callback_errors_and_wrong_namespace_propagate_without_ordinal_or_scan_retry() {
    for mode in [Mode::Cancelled, Mode::WrongPolicy] {
        let resolver = Resolver {
            calls: Cell::new(0),
            mode,
        };
        let error = run(
            "SELECT K,Count(*) FROM T GROUP BY K",
            &source(&["a", "b"]),
            &resolver,
        )
        .unwrap_err();
        assert!(matches!(
            (mode, error),
            (Mode::Cancelled, QueryError::Cancelled) | (Mode::WrongPolicy, QueryError::Source)
        ));
        assert_eq!(resolver.calls.get(), 1);
    }
}

#[test]
fn mixed_policy_cohort_and_late_eager_key_errors_are_not_hidden_by_hints() {
    let resolver = Resolver {
        calls: Cell::new(0),
        mode: Mode::Collide,
    };
    assert!(matches!(
        run(
            "SELECT K,Count(*) FROM (SELECT K FROM T UNION ALL SELECT K FROM U) AS Q GROUP BY K",
            &source(&["a"]),
            &resolver
        ),
        Err(QueryError::CollationConflict)
    ));
    assert_eq!(resolver.calls.get(), 0);
    assert!(
        run(
            "SELECT Count(*) FROM T GROUP BY IIf(K='bad',1/0,K)",
            &source(&["a", "bad"]),
            &resolver
        )
        .is_err()
    );
    assert_eq!(resolver.calls.get(), 0);
}
