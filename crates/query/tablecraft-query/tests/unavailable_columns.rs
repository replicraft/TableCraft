use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_schema::{Column, ScalarType, SchemaError, SchemaName};
use tablecraft_sql::{ParseLimits, parse_query};

struct PartialSource {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    rows: Vec<Vec<Value>>,
    changed: Cell<bool>,
    scans: Cell<usize>,
}
impl PartialSource {
    fn new(opaque_only: bool) -> Self {
        Self {
            columns: if opaque_only {
                vec![]
            } else {
                vec![Column::new("ID".into(), ScalarType::Int64, false).unwrap()]
            },
            unavailable: vec![SchemaName::new("Files".into()).unwrap()],
            rows: (1..=3)
                .map(|id| {
                    if opaque_only {
                        vec![]
                    } else {
                        vec![Value::Int64(id)]
                    }
                })
                .collect(),
            changed: Cell::new(false),
            scans: Cell::new(0),
        }
    }
}
impl Source for PartialSource {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(if self.changed.get() {
            &[]
        } else {
            &self.unavailable
        })
    }
    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
}
fn execute(sql: &str, source: &dyn Source) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn scalar_bindings_and_aggregates_work_without_placeholder_nulls() {
    let source = PartialSource::new(false);
    assert_eq!(
        execute(
            "SELECT ID FROM Mixed WHERE ID > 1 ORDER BY ID DESC",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(3)], vec![Value::Int64(2)]]
    );
    assert_eq!(
        execute("SELECT Count(*), Sum(ID) FROM Mixed", &source).unwrap(),
        [vec![Value::Int64(3), Value::Int64(6)]]
    );
    assert_eq!(execute("SELECT M.ID, (SELECT Count(*) FROM Other AS O WHERE O.ID=M.ID) FROM Mixed AS M ORDER BY M.ID", &source).unwrap(), [vec![Value::Int64(1), Value::Int64(1)],vec![Value::Int64(2), Value::Int64(1)],vec![Value::Int64(3), Value::Int64(1)]]);
}

#[test]
fn unavailable_references_wildcards_and_ambiguity_fail_before_scanning() {
    let source = PartialSource::new(false);
    for sql in [
        "SELECT * FROM Mixed",
        "SELECT M.* FROM Mixed AS M",
        "SELECT Files FROM Mixed",
        "SELECT M.Files FROM Mixed AS M",
        "SELECT ID FROM Mixed WHERE Files IS NULL",
        "SELECT ID FROM Mixed ORDER BY Files",
        "SELECT (SELECT M.Files FROM Other) FROM Mixed AS M",
    ] {
        assert_eq!(execute(sql, &source), Err(QueryError::Unsupported), "{sql}");
    }
    assert_eq!(
        execute(
            "SELECT Files FROM Mixed AS A INNER JOIN Mixed AS B ON A.ID=B.ID",
            &source
        ),
        Err(QueryError::AmbiguousName)
    );
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn join_padding_and_derived_aliases_use_only_available_row_widths() {
    let source = PartialSource::new(false);
    for kind in ["LEFT", "RIGHT"] {
        let sql = format!(
            "SELECT A.ID, B.ID FROM Mixed AS A {kind} JOIN Mixed AS B ON A.ID=B.ID+10 ORDER BY {}.ID",
            if kind == "LEFT" { "A" } else { "B" }
        );
        let expected = (1..=3)
            .map(|id| {
                if kind == "LEFT" {
                    vec![Value::Int64(id), Value::Null]
                } else {
                    vec![Value::Null, Value::Int64(id)]
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(execute(&sql, &source).unwrap(), expected);
    }
    assert_eq!(execute("SELECT D.* FROM Mixed AS M INNER JOIN (SELECT ID FROM Other) AS D ON M.ID=D.ID ORDER BY D.ID", &source).unwrap(), [vec![Value::Int64(1)],vec![Value::Int64(2)],vec![Value::Int64(3)]]);
}

#[test]
fn opaque_only_sources_allow_row_counts_but_never_empty_wildcards() {
    let source = PartialSource::new(true);
    assert_eq!(
        execute("SELECT Count(*) FROM Opaque", &source).unwrap(),
        [vec![Value::Int64(3)]]
    );
    assert_eq!(
        execute("SELECT * FROM Opaque", &source),
        Err(QueryError::Unsupported)
    );
}

#[test]
fn unavailable_schema_changes_and_overlapping_declarations_are_not_hidden() {
    let mut source = PartialSource::new(false);
    let parsed = parse_query("SELECT ID FROM Mixed", ParseLimits::default()).unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    source.changed.set(true);
    assert!(matches!(
        prepared.execute(&source, &[], ExecutionOptions::default()),
        Err(QueryError::SchemaChanged)
    ));
    assert_eq!(source.scans.get(), 0);
    source.changed.set(false);
    source
        .unavailable
        .push(SchemaName::new("id".into()).unwrap());
    assert_eq!(
        execute("SELECT ID FROM Mixed", &source),
        Err(QueryError::Schema(SchemaError::Duplicate))
    );
    source.unavailable = vec![
        SchemaName::new("Files".into()).unwrap(),
        SchemaName::new("FILES".into()).unwrap(),
    ];
    assert_eq!(
        execute("SELECT ID FROM Mixed", &source),
        Err(QueryError::Schema(SchemaError::Duplicate))
    );
}

#[test]
fn unavailable_fields_never_accept_extra_null_placeholders_in_source_rows() {
    let mut source = PartialSource::new(false);
    source.rows[0].push(Value::Null);
    assert_eq!(
        execute("SELECT ID FROM Mixed", &source),
        Err(QueryError::Schema(SchemaError::Row))
    );
}
