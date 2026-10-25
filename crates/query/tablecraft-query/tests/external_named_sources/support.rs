use std::{cell::Cell, rc::Rc};
use tablecraft_core::values::Value;
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryResult, Source, prepare,
    source::{
        ExternalSourceRequest, MemorySource, MemoryTable, ResolvedExternalSource, SourceKey,
        TextCollationId,
    },
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

pub struct Node {
    data: MemorySource,
    pub definitions: Vec<(String, Query)>,
    pub scans: Cell<usize>,
    pub fail: Cell<bool>,
    pub change_revision: Cell<bool>,
    revision: Rc<Cell<u128>>,
    pub policy: Vec<Option<TextCollationId>>,
}

impl Node {
    fn new(id: i32, policy: u64, revision: Rc<Cell<u128>>) -> Self {
        let columns = vec![
            Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
            Column::new("Label".into(), ScalarType::Text, false).unwrap(),
            Column::new("Formula".into(), ScalarType::Text, false).unwrap(),
        ];
        let rows = vec![vec![
            Value::Int32(id),
            Value::Text(format!("node{id}")),
            Value::Text(format!("{id}+100")),
        ]];
        let table = MemoryTable::new("T".into(), columns, rows).unwrap();
        let data = MemorySource::new(vec![table]).unwrap();
        let mut node = Self {
            data,
            definitions: Vec::new(),
            scans: Cell::new(0),
            fail: Cell::new(false),
            change_revision: Cell::new(false),
            revision,
            policy: vec![None, Some(TextCollationId::from_host(policy)), None],
        };
        node.define("Same", "SELECT Id,Label FROM T");
        node.define("Nested", "SELECT Id,Label FROM Same");
        node.define(
            "D",
            "TRANSFORM Count(*) SELECT 0 AS G FROM T GROUP BY 0 PIVOT Id",
        );
        node.define(
            "F",
            "TRANSFORM Count(*) SELECT 0 AS G FROM T GROUP BY 0 PIVOT Id IN(1,2,10)",
        );
        node
    }

    pub fn define(&mut self, name: &str, sql: &str) {
        let query = parse_query(sql, Default::default()).unwrap().into_query();
        self.definitions.push((name.into(), query));
    }
}

impl Source for Node {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .definitions
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        self.data.columns(table)?;
        Ok(&self.policy)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(table, visitor)?;
        if self.change_revision.replace(false) {
            self.revision.set(self.revision.get() + 10);
        }
        if self.fail.get() {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}

pub struct Host {
    pub root: Node,
    pub one: Node,
    pub two: Node,
    pub revision: Rc<Cell<u128>>,
    pub enabled: Cell<bool>,
}

impl Host {
    pub fn new() -> Self {
        let revision = Rc::new(Cell::new(1));
        Self {
            root: Node::new(10, 10, revision.clone()),
            one: Node::new(1, 11, revision.clone()),
            two: Node::new(2, 12, revision.clone()),
            revision,
            enabled: Cell::new(true),
        }
    }

    pub fn scans(&self) -> usize {
        self.root.scans.get() + self.one.scans.get() + self.two.scans.get()
    }
}

impl Source for Host {
    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        if !self.enabled.get() || request.connection().is_some() || request.connection_bracketed() {
            return Ok(None);
        }
        Ok(match request.database() {
            "one" => Some(ResolvedExternalSource::new(
                SourceKey::new(self.revision.get()),
                &self.one,
            )),
            "two" => Some(ResolvedExternalSource::new(SourceKey::new(2), &self.two)),
            _ => None,
        })
    }

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
}

pub fn options() -> ExecutionOptions<'static> {
    ExecutionOptions {
        number_format: Some(NumberFormat::Invariant),
        ..Default::default()
    }
}

pub fn run(host: &Host, sql: &str) -> Result<QueryResult, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), host)?.execute(host, &[], options())
}
