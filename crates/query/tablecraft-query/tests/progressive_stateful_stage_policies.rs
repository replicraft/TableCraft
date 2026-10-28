//! Independent logical EOF, retained provenance and lexical ownership boundaries.
#[path = "progressive_stateful_stage_policies/lifecycle.rs"]
mod lifecycle;
#[path = "progressive_stateful_stage_policies/provenance.rs"]
mod provenance;

use std::{cell::Cell, cmp::Ordering, rc::Rc};
use tablecraft_core::values::Value;
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, QueryLimits, StreamError, prepare,
    prepare_with_options,
    source::{
        CollationResolver, ExternalSourceRequest, ResolvedExternalSource, Source, SourceKey,
        TextCollationId,
    },
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

const FAILING_INNER: &str =
    "(SELECT G,Count(*) AS N,Min(A) AS S,1/(G-2) AS Bad FROM T GROUP BY G) Q";

struct Node {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    definitions: Vec<(String, Query)>,
    active: Rc<Cell<bool>>,
    eof: Cell<bool>,
    scans: Cell<usize>,
    buffered_scans: Cell<usize>,
    fail_eof: bool,
    swallow: bool,
}

impl Node {
    fn new(first: i32, active: Rc<Cell<bool>>) -> Self {
        let mut node = Self {
            columns: [
                ("Id", ScalarType::Int32),
                ("G", ScalarType::Int32),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
                ("C", ScalarType::Text),
            ]
            .map(|(name, kind)| Column::new(name.into(), kind, true).unwrap())
            .to_vec(),
            rows: [(first, 1, "ab"), (first + 1, 2, "yz")]
                .into_iter()
                .map(|(identity, group, text)| {
                    vec![
                        Value::Int32(identity),
                        Value::Int32(group),
                        Value::Text("unused".into()),
                        Value::Text("15".into()),
                        Value::Text(text.into()),
                    ]
                })
                .collect(),
            policies: vec![
                None,
                None,
                Some(TextCollationId::from_host(999)),
                Some(TextCollationId::from_host(2)),
                Some(TextCollationId::from_host(3)),
            ],
            definitions: Vec::new(),
            active,
            eof: Cell::new(false),
            scans: Cell::new(0),
            buffered_scans: Cell::new(0),
            fail_eof: false,
            swallow: false,
        };
        node.define("Same", "SELECT Count(*) AS N,Min(Id) AS M FROM T");
        node.define("Nested", "SELECT N,M FROM Same");
        node
    }

    fn define(&mut self, name: &str, sql: &str) {
        let query = parse_query(sql, Default::default()).unwrap().into_query();
        self.definitions.push((name.into(), query));
    }
}

impl Source for Node {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        assert!(!self.active.get(), "named metadata inside exclusive cursor");
        Ok(self
            .definitions
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        assert!(
            !self.active.get(),
            "column metadata inside exclusive cursor"
        );
        if table != "T" {
            return Err(QueryError::UnknownName);
        }
        Ok(&self.columns)
    }

    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        assert!(!self.active.get());
        Ok(&self.policies)
    }

    fn external_source(
        &self,
        _request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        panic!("explicit selector must use operation root registry")
    }

    fn scan(
        &self,
        _table: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.buffered_scans.set(self.buffered_scans.get() + 1);
        panic!("stateful pipeline must not materialize a source snapshot")
    }

    fn scan_projected_progressive(
        &self,
        _table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(!self.active.replace(true));
        self.eof.set(false);
        self.scans.set(self.scans.get() + 1);
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

struct Host {
    root: Node,
    one: Node,
    two: Node,
    active: Rc<Cell<bool>>,
}

impl Host {
    fn new() -> Self {
        let active = Rc::new(Cell::new(false));
        Self {
            root: Node::new(10, active.clone()),
            one: Node::new(1, active.clone()),
            two: Node::new(100, active.clone()),
            active,
        }
    }

    fn assert_no_snapshot(&self) {
        assert_eq!(self.root.buffered_scans.get(), 0);
        assert_eq!(self.one.buffered_scans.get(), 0);
        assert_eq!(self.two.buffered_scans.get(), 0);
    }
}

impl Source for Host {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        self.root.query(name)
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.root.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        self.root.text_collations(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.root.scan(table, visitor)
    }

    fn scan_projected_progressive(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.root
            .scan_projected_progressive(table, indices, visitor)
    }

    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        assert!(
            !self.active.get(),
            "registry metadata inside exclusive cursor"
        );
        if request.connection().is_some() || request.connection_bracketed() {
            return Ok(None);
        }
        Ok(match request.database() {
            "one" => Some(ResolvedExternalSource::new(SourceKey::new(11), &self.one)),
            "two" => Some(ResolvedExternalSource::new(SourceKey::new(22), &self.two)),
            _ => None,
        })
    }
}

struct Order(bool);

impl Collation for Order {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(if self.0 {
            Ordering::Equal
        } else {
            left.cmp(right).reverse()
        })
    }
}

static EQUAL: Order = Order(true);
static REVERSE: Order = Order(false);

struct Resolver<'a> {
    host: &'a Host,
    hits: Cell<usize>,
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(!self.host.active.get());
        assert!(self.host.root.eof.get());
        self.hits.set(self.hits.get() + 1);
        match id.host_key() {
            2 => Ok(&EQUAL),
            3 => Ok(&REVERSE),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral: declared,
        number_format: Some(NumberFormat::Invariant),
        ..Default::default()
    }
}

fn stream(
    host: &Host,
    sql: &str,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), host).unwrap();
    let mut rows = Vec::new();
    let result = plan.execute_progressive(host, &[], options, &mut |row| {
        assert!(!host.active.get());
        rows.push(row.to_vec());
        Ok(())
    });
    host.assert_no_snapshot();
    result?;
    Ok(rows)
}
