use crate::{
    OutputType, QueryError, Source,
    binding::{PreparedQuery, prepare},
    plan::{PreparedBody, ReadNode, RelationNode},
    source::RecordsetColumn,
};
use std::{cell::Cell, sync::Arc};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{MemberAvailability, RecordsetSchema};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaName};
use tablecraft_sql::{ParseLimits, parse_query};

struct Metadata {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    recordsets: Vec<RecordsetColumn>,
    scans: Cell<usize>,
}

impl Metadata {
    fn new() -> Self {
        let schema = Arc::new(
            RecordsetSchema::new(
                vec![
                    Column::new("FileName".into(), ScalarType::Text, true).unwrap(),
                    Column::new("FileData".into(), ScalarType::Binary, true).unwrap(),
                ],
                vec![
                    MemberAvailability::Available,
                    MemberAvailability::Uninspected,
                ],
                NameComparison::AsciiInsensitive,
                &mut ResourceBudget::new(0, 0, 100),
                &|| false,
            )
            .unwrap(),
        );
        Self {
            columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            unavailable: ["Files", "Other"]
                .map(|name| SchemaName::new(name.into()).unwrap())
                .into(),
            recordsets: vec![
                RecordsetColumn::new("Files".into(), Arc::clone(&schema)).unwrap(),
                RecordsetColumn::new("Other".into(), schema).unwrap(),
            ],
            scans: Cell::new(0),
        }
    }
}

impl Source for Metadata {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }

    fn recordset_columns(&self, _: &str) -> Result<&[RecordsetColumn], QueryError> {
        Ok(&self.recordsets)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        Ok(())
    }
}

fn bind(sql: &str, source: &Metadata) -> Result<PreparedQuery, QueryError> {
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(query.query(), source)
}

#[test]
fn captured_recordset_schemas_are_sparse_and_do_not_scan_or_infer_child_rows() {
    let source = Metadata::new();
    let prepared = bind("SELECT Id, Other AS Documents FROM T", &source).unwrap();
    assert_eq!(prepared.output_names().unwrap(), ["Id", "Documents"]);
    assert!(prepared.recordset_schema(0).is_none());
    let child = prepared.recordset_schema(1).unwrap();
    assert_eq!(child.columns()[0].name(), "FileName");
    assert_eq!(child.availability()[1], MemberAvailability::Uninspected);
    assert_eq!(
        prepared.output_types().unwrap()[1],
        OutputType::Recordset {
            parent_nullable: false
        }
    );
    let PreparedBody::Read(read) = &prepared.body else {
        panic!("ordinary read")
    };
    assert_eq!(read.recordset_schemas.len(), 1);
    let ReadNode::Select(select) = &read.node else {
        panic!("SELECT")
    };
    let RelationNode::Table {
        recordset_projection,
        ..
    } = &select.source.node
    else {
        panic!("table")
    };
    assert_eq!(recordset_projection, &[1]);
    assert_eq!(source.scans.get(), 0);

    let scalar = bind("SELECT Id FROM T", &source).unwrap();
    let PreparedBody::Read(read) = &scalar.body else {
        panic!("ordinary read")
    };
    assert!(read.recordset_schemas.is_empty());
    let ReadNode::Select(select) = &read.node else {
        panic!("SELECT")
    };
    let RelationNode::Table {
        recordset_projection,
        ..
    } = &select.source.node
    else {
        panic!("table")
    };
    assert!(recordset_projection.is_empty());
}

#[test]
fn wildcard_and_derived_outputs_retain_schemas_without_scalar_positions() {
    let source = Metadata::new();
    let wildcard = bind("SELECT * FROM T", &source).unwrap();
    assert_eq!(wildcard.output_names().unwrap(), ["Id", "Files", "Other"]);
    assert!(wildcard.recordset_schema(1).is_some());
    assert!(wildcard.recordset_schema(2).is_some());
    let derived = bind(
        "SELECT d.Documents, d.Id FROM (SELECT Id, Files AS Documents FROM T) AS d",
        &source,
    )
    .unwrap();
    assert!(derived.recordset_schema(0).is_some());
    assert!(derived.recordset_schema(1).is_none());
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn recordset_scalar_key_and_group_operations_refuse_before_any_scan() {
    let source = Metadata::new();
    for sql in [
        "SELECT Files + 1 FROM T",
        "SELECT Files = Files FROM T",
        "SELECT Files FROM T ORDER BY 1",
        "SELECT DISTINCT Files FROM T",
        "SELECT Files FROM T GROUP BY Files",
        "SELECT Files FROM T UNION ALL SELECT Files FROM T",
        "SELECT (SELECT Files FROM T) FROM T",
    ] {
        assert!(bind(sql, &source).is_err(), "{sql}");
        assert_eq!(source.scans.get(), 0, "{sql}");
    }
}

#[test]
fn default_source_refuses_requested_recordsets_before_invoking_scalar_scan() {
    let source = Metadata::new();
    assert!(matches!(
        source.scan_structured("T", &[0], &[], &[], &[0], &mut |_, _, _| panic!(
            "no callback"
        )),
        Err(QueryError::Unsupported)
    ));
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn recordset_names_must_be_unique_unavailable_source_names() {
    let mut source = Metadata::new();
    source.recordsets[1] =
        RecordsetColumn::new("FILES".into(), Arc::clone(source.recordsets[0].schema())).unwrap();
    assert!(bind("SELECT Id FROM T", &source).is_err());
    source.recordsets[1] =
        RecordsetColumn::new("Id".into(), Arc::clone(source.recordsets[0].schema())).unwrap();
    assert!(bind("SELECT Id FROM T", &source).is_err());
    assert_eq!(source.scans.get(), 0);
}
