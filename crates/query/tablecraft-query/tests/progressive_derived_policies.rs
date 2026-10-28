//! Independent namespace, retained-cell and provider controls for static pipelines.
#[path = "progressive_derived_policies/namespaces.rs"]
mod namespaces;
#[path = "progressive_derived_policies/retained.rs"]
mod retained;

use std::{cell::Cell, cmp::Ordering, rc::Rc};
use tablecraft_core::values::Value;
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, StreamError, prepare, prepare_with_options,
    source::{
        CollationResolver, ExternalSourceRequest, ResolvedExternalSource, Source, SourceKey,
        TextCollationId,
    },
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

struct Node {
    columns: Vec<Column>,
    changed: Vec<Column>,
    rows: Vec<Vec<Value>>,
    definitions: Vec<(String, Query)>,
    policies: Vec<Option<TextCollationId>>,
    active: Rc<Cell<bool>>,
    eof: Cell<bool>,
    scans: Cell<usize>,
    schema_changed: Rc<Cell<bool>>,
    mutate_after_scan: Option<Rc<Cell<bool>>>,
    remap_on_columns: Cell<bool>,
    remap_target: Option<Rc<Cell<bool>>>,
    remaps: Cell<usize>,
}

impl Node {
    fn new(first_id: i32, policy: u64, active: Rc<Cell<bool>>) -> Self {
        let columns = [
            ("Id", ScalarType::Int32),
            ("A", ScalarType::Text),
            ("B", ScalarType::Text),
        ]
        .map(|(name, kind)| Column::new(name.into(), kind, true).unwrap())
        .to_vec();
        let mut node = Self {
            changed: vec![Column::new("Changed".into(), ScalarType::Int32, true).unwrap()],
            columns,
            rows: (first_id..first_id + 3)
                .map(|identity| {
                    vec![
                        Value::Int32(identity),
                        Value::Text("unused".into()),
                        Value::Text("15".into()),
                    ]
                })
                .collect(),
            definitions: Vec::new(),
            policies: vec![
                None,
                Some(TextCollationId::from_host(policy)),
                Some(TextCollationId::from_host(policy + 10)),
            ],
            active,
            eof: Cell::new(false),
            scans: Cell::new(0),
            schema_changed: Rc::new(Cell::new(false)),
            mutate_after_scan: None,
            remap_on_columns: Cell::new(false),
            remap_target: None,
            remaps: Cell::new(0),
        };
        node.define("Same", "SELECT Id,A,B FROM T");
        node.define("Nested", "SELECT Id,A,B FROM Same");
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
        if self.remap_on_columns.replace(false) {
            self.remap_target.as_ref().unwrap().set(true);
            self.remaps.set(self.remaps.get() + 1);
        }
        Ok(if self.schema_changed.get() {
            &self.changed
        } else {
            &self.columns
        })
    }

    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        assert!(!self.active.get());
        Ok(&self.policies)
    }

    fn external_source(
        &self,
        _request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        panic!("explicit selectors must use the operation root registry")
    }

    fn scan(
        &self,
        _table: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("static pipeline must not fall back to a buffered source scan")
    }

    fn scan_projected_progressive(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert_eq!(table, "T");
        assert!(!self.active.replace(true));
        self.eof.set(false);
        self.scans.set(self.scans.get() + 1);
        let result = (|| {
            for row in &self.rows {
                visitor(ProjectedRow::select(row, indices)?)?;
            }
            self.eof.set(true);
            Ok(())
        })();
        self.active.set(false);
        if let Some(changed) = &self.mutate_after_scan {
            changed.set(true);
        }
        result
    }
}

struct Host {
    root: Node,
    one: Node,
    two: Node,
    active: Rc<Cell<bool>>,
    key: Cell<u128>,
    remapped: Rc<Cell<bool>>,
    binding_lookups: Cell<usize>,
}

impl Host {
    fn new() -> Self {
        let active = Rc::new(Cell::new(false));
        let remapped = Rc::new(Cell::new(false));
        let mut one = Node::new(1, 2, active.clone());
        one.remap_target = Some(remapped.clone());
        Self {
            root: Node::new(10, 1, active.clone()),
            one,
            two: Node::new(100, 3, active.clone()),
            active,
            key: Cell::new(11),
            remapped,
            binding_lookups: Cell::new(0),
        }
    }

    fn scans(&self) -> usize {
        self.root.scans.get() + self.one.scans.get() + self.two.scans.get()
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

    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        assert!(
            !self.active.get(),
            "external metadata inside exclusive cursor"
        );
        self.binding_lookups.set(self.binding_lookups.get() + 1);
        if request.connection().is_some() || request.connection_bracketed() {
            return Ok(None);
        }
        Ok(match request.database() {
            "one" => Some(ResolvedExternalSource::new(
                SourceKey::new(self.key.get()),
                if self.remapped.get() {
                    &self.two
                } else {
                    &self.one
                },
            )),
            "two" => Some(ResolvedExternalSource::new(SourceKey::new(22), &self.two)),
            _ => None,
        })
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
    let mut output = Vec::new();
    plan.execute_progressive(host, &[], options, &mut |row| {
        output.push(row.to_vec());
        Ok(())
    })?;
    Ok(output)
}
