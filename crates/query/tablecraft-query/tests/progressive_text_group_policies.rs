//! Independent final-key versus aggregate-policy populations and phase boundaries.
#[path = "progressive_text_group_policies/cohorts.rs"]
mod cohorts;
#[path = "progressive_text_group_policies/phases.rs"]
mod phases;

use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    rc::Rc,
};
use tablecraft_core::values::Value;
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, StreamError, prepare,
    source::{CollationResolver, Source, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

const KEY: &str = "IIf(Id=3,B,Id)&IIf(True,0,A)";

struct Data {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    eof: Cell<bool>,
    active: Cell<bool>,
    buffered_scans: Cell<usize>,
    fail_eof: bool,
    swallow: bool,
}

impl Data {
    fn new() -> Self {
        Self {
            columns: [
                ("Id", ScalarType::Int32),
                ("N", ScalarType::Int64),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
                ("C", ScalarType::Text),
            ]
            .map(|(name, kind)| Column::new(name.into(), kind, true).unwrap())
            .to_vec(),
            rows: [(1, "ab"), (2, "yz"), (3, "mn")]
                .into_iter()
                .map(|(identity, text)| {
                    vec![
                        Value::Int32(identity),
                        Value::Int64(i64::from(identity)),
                        Value::Text("unused".into()),
                        Value::Text("15".into()),
                        Value::Text(text.into()),
                    ]
                })
                .collect(),
            policies: vec![
                None,
                None,
                Some(TextCollationId::from_host(1)),
                Some(TextCollationId::from_host(2)),
                Some(TextCollationId::from_host(3)),
            ],
            eof: Cell::new(false),
            active: Cell::new(false),
            buffered_scans: Cell::new(0),
            fail_eof: false,
            swallow: false,
        }
    }
}

impl Source for Data {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
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
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.buffered_scans.set(self.buffered_scans.get() + 1);
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
        assert!(!self.active.replace(true));
        self.eof.set(false);
        let result = (|| {
            for row in &self.rows {
                let result = visitor(ProjectedRow::select(row, indices)?);
                if !self.swallow {
                    result?;
                }
            }
            if self.fail_eof {
                return Err(QueryError::Source);
            }
            self.eof.set(true);
            Ok(())
        })();
        self.active.set(false);
        result
    }
}

#[derive(Clone, Copy, Debug)]
enum Event {
    Resolve(u64),
    Compare(u64),
}

struct Provider<'a> {
    source: &'a Data,
    trace: Rc<RefCell<Vec<Event>>>,
    id: u64,
}

impl Collation for Provider<'_> {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        assert!(self.source.eof.get());
        assert!(!self.source.active.get());
        self.trace.borrow_mut().push(Event::Compare(self.id));
        match self.id {
            2 => Ok(Ordering::Equal),
            3 => Ok(left.cmp(right).reverse()),
            4 if left.contains("bad") || right.contains("bad") => Err(EvalError::Domain),
            4 => Ok(left.cmp(right)),
            _ => unreachable!("only authored providers are constructed"),
        }
    }
}

struct Resolver<'a> {
    source: &'a Data,
    trace: Rc<RefCell<Vec<Event>>>,
    equal: Provider<'a>,
    reverse: Provider<'a>,
    failing: Provider<'a>,
}

impl<'a> Resolver<'a> {
    fn new(source: &'a Data) -> Self {
        let trace = Rc::new(RefCell::new(Vec::new()));
        Self {
            source,
            trace: trace.clone(),
            equal: Provider {
                source,
                trace: trace.clone(),
                id: 2,
            },
            reverse: Provider {
                source,
                trace: trace.clone(),
                id: 3,
            },
            failing: Provider {
                source,
                trace,
                id: 4,
            },
        }
    }
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(self.source.eof.get());
        assert!(!self.source.active.get());
        self.trace.borrow_mut().push(Event::Resolve(id.host_key()));
        match id.host_key() {
            2 => Ok(&self.equal),
            3 => Ok(&self.reverse),
            4 => Ok(&self.failing),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}

fn options<'a>(resolver: &'a Resolver<'a>, declared: bool) -> ExecutionOptions<'a> {
    ExecutionOptions {
        declared_integral: declared,
        number_format: Some(NumberFormat::Invariant),
        collation_resolver: Some(resolver),
        ..Default::default()
    }
}

fn stream(
    source: &Data,
    sql: &str,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let mut rows = Vec::new();
    let buffered = source.buffered_scans.get();
    let result = plan.execute_progressive(source, &[], options, &mut |row| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        rows.push(row.to_vec());
        Ok(())
    });
    assert_eq!(source.buffered_scans.get(), buffered);
    result?;
    Ok(rows)
}
