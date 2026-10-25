use std::{cell::Cell, rc::Rc};
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{
        ExternalSourceRequest, MemorySource, MemoryTable, MultivalueList, MultivalueVisitor,
        ProjectedRow, ResolvedExternalSource, ScanConstraint, SourceKey, TextCollationId,
    },
};
use tablecraft_schema::{CollectionMember, Column, ScalarType, SchemaName};
use tablecraft_sql::{parse_query, query::Query};

pub struct List(Vec<Value>);

impl MultivalueList for List {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn get(&self, index: usize) -> Option<&Value> {
        self.0.get(index)
    }
}

pub struct Provider {
    pub data: MemorySource,
    pub scans: Cell<usize>,
    pub projections: Cell<usize>,
    pub collection_scans: Cell<usize>,
    pub hint_calls: Cell<usize>,
    pub saw_constraint: Cell<bool>,
    pub named_calls: Cell<usize>,
    pub rows_allowed: Cell<bool>,
    pub fail_late: Cell<bool>,
    pub changed_schema: Cell<bool>,
    pub change_key_after_scan: Cell<bool>,
    pub key: Rc<Cell<u128>>,
    pub policy: Vec<Option<TextCollationId>>,
    pub unavailable: Vec<SchemaName>,
    pub values: Vec<Column>,
    pub members: Vec<CollectionMember>,
    lists: [List; 3],
}

impl Provider {
    fn new(ids: &[i32], policy: u64, key: Rc<Cell<u128>>, collections: bool) -> Self {
        let columns = || {
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, false).unwrap(),
            ]
        };
        let data = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                columns(),
                ids.iter()
                    .map(|id| vec![Value::Int32(*id), Value::Text(format!("value{id}"))])
                    .collect(),
            )
            .unwrap(),
            MemoryTable::new(
                "U".into(),
                vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
                vec![vec![Value::Int32(ids[0])]],
            )
            .unwrap(),
        ])
        .unwrap();

        Self {
            data,
            scans: Cell::new(0),
            projections: Cell::new(0),
            collection_scans: Cell::new(0),
            hint_calls: Cell::new(0),
            saw_constraint: Cell::new(false),
            named_calls: Cell::new(0),
            rows_allowed: Cell::new(false),
            fail_late: Cell::new(false),
            changed_schema: Cell::new(false),
            change_key_after_scan: Cell::new(false),
            key,
            policy: vec![None, Some(TextCollationId::from_host(policy))],
            unavailable: if collections {
                vec![
                    SchemaName::new("Tags".into()).unwrap(),
                    SchemaName::new("Files".into()).unwrap(),
                ]
            } else {
                vec![]
            },
            values: if collections {
                vec![Column::new("Tags".into(), ScalarType::Text, false).unwrap()]
            } else {
                vec![]
            },
            members: if collections {
                vec![
                    CollectionMember::new(
                        "Files".into(),
                        Column::new("Name".into(), ScalarType::Text, false).unwrap(),
                    )
                    .unwrap(),
                    CollectionMember::new(
                        "Files".into(),
                        Column::new("Size".into(), ScalarType::Int32, false).unwrap(),
                    )
                    .unwrap(),
                ]
            } else {
                vec![]
            },
            lists: [
                List(vec![Value::Text("red".into()), Value::Text("blue".into())]),
                List(vec![Value::Text("n".into()), Value::Text("m".into())]),
                List(vec![Value::Int32(10), Value::Int32(20)]),
            ],
        }
    }

    fn completed(&self, result: Result<(), QueryError>) -> Result<(), QueryError> {
        result?;

        if self.fail_late.get() {
            return Err(QueryError::Source);
        }

        if self.change_key_after_scan.replace(false) {
            self.key.set(self.key.get() + 1);
        }

        Ok(())
    }
}

impl Source for Provider {
    fn query(&self, _: &str) -> Result<Option<&Query>, QueryError> {
        self.named_calls.set(self.named_calls.get() + 1);
        Err(QueryError::Source)
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data
            .columns(if table == "T" && self.changed_schema.get() {
                "U"
            } else {
                table
            })
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(if table == "T" { &self.policy } else { &[] })
    }

    fn unavailable_columns(&self, table: &str) -> Result<&[SchemaName], QueryError> {
        Ok(if table == "T" { &self.unavailable } else { &[] })
    }

    fn multivalue_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        Ok(if table == "T" { &self.values } else { &[] })
    }

    fn collection_members(&self, table: &str) -> Result<&[CollectionMember], QueryError> {
        Ok(if table == "T" { &self.members } else { &[] })
    }

    fn text_equality_hints(&self, _: &str) -> Result<bool, QueryError> {
        self.hint_calls.set(self.hint_calls.get() + 1);
        Ok(true)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(self.rows_allowed.get(), "binding scanned external records");
        self.scans.set(self.scans.get() + 1);
        self.completed(self.data.scan(table, visitor))
    }

    fn scan_projected(
        &self,
        table: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(self.rows_allowed.get(), "binding scanned external records");

        self.projections.set(self.projections.get() + 1);
        self.saw_constraint.set(constraint.is_some());
        // Deliberately ignore the safe hint: the executor must keep its filter.
        self.completed(self.data.scan_projected(table, indices, None, visitor))
    }

    fn scan_collection_members(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        assert!(self.rows_allowed.get(), "binding scanned external children");
        self.collection_scans.set(self.collection_scans.get() + 1);

        let mut lists: Vec<&dyn MultivalueList> = values
            .iter()
            .map(|_| &self.lists[0] as &dyn MultivalueList)
            .collect();

        lists.extend(
            members
                .iter()
                .map(|index| &self.lists[index + 1] as &dyn MultivalueList),
        );

        self.completed(
            self.data
                .scan_projected(table, indices, None, &mut |row| visitor(row, &lists)),
        )
    }
}

pub struct Host {
    pub local: Provider,
    pub remote: Provider,
    pub second: Provider,
    pub enabled: Cell<bool>,
    pub lookups: Cell<usize>,
    pub key: Rc<Cell<u128>>,
    pub definitions: Vec<(String, Query)>,
    pub exact_connection: Option<String>,
    pub bracketed: bool,
}

impl Host {
    pub fn new(collections: bool) -> Self {
        let key = Rc::new(Cell::new(7));

        Self {
            local: Provider::new(&[10, 20], 1, key.clone(), false),
            remote: Provider::new(&[1, 2], 2, key.clone(), collections),
            second: Provider::new(&[100, 200], 3, key.clone(), false),
            enabled: Cell::new(true),
            lookups: Cell::new(0),
            key,
            definitions: Vec::new(),
            exact_connection: None,
            bracketed: false,
        }
    }

    pub fn allow_rows(&self) {
        self.local.rows_allowed.set(true);
        self.remote.rows_allowed.set(true);
        self.second.rows_allowed.set(true);
    }

    pub fn definition(&mut self, name: &str, sql: &str) {
        self.definitions.push((
            name.into(),
            parse_query(sql, Default::default())
                .unwrap()
                .query()
                .clone(),
        ));
    }
}

impl Source for Host {
    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        self.lookups.set(self.lookups.get() + 1);
        if !self.enabled.get()
            || request.connection() != self.exact_connection.as_deref()
            || request.connection_bracketed() != self.bracketed
        {
            return Ok(None);
        }

        Ok(match request.database() {
            "remote" => Some(ResolvedExternalSource::new(
                SourceKey::new(self.key.get()),
                &self.remote,
            )),
            "second" => Some(ResolvedExternalSource::new(
                SourceKey::new(99),
                &self.second,
            )),
            _ => None,
        })
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .definitions
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.local.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        self.local.text_collations(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.local.scan(table, visitor)
    }
}

pub fn run(host: &Host, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), host)?;

    host.allow_rows();

    Ok(plan
        .execute(host, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
