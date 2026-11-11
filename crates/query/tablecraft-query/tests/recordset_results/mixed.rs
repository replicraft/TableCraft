use super::source::Data;
use std::sync::Arc;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_db::{MultivalueList, ProjectedRow, StoredCell};
use tablecraft_query::{
    QueryError, QueryValue, Source,
    source::{MultivalueVisitor, RecordsetColumn, StructuredVisitor},
};
use tablecraft_schema::{CollectionMember, Column, ScalarType, SchemaName};

struct Mixed {
    data: Data,
    unavailable: Vec<SchemaName>,
    values: Vec<Column>,
    members: Vec<CollectionMember>,
    tags: Vec<Vec<Value>>,
    names: Vec<Vec<Value>>,
}

impl Mixed {
    fn new() -> Self {
        Self {
            data: Data::new(),
            unavailable: ["Files", "Tags"]
                .map(|name| SchemaName::new(name.into()).unwrap())
                .into(),
            values: vec![Column::new("Tags".into(), ScalarType::Int32, false).unwrap()],
            members: vec![
                CollectionMember::new(
                    "Files".into(),
                    Column::new("FileName".into(), ScalarType::Text, true).unwrap(),
                )
                .unwrap(),
            ],
            tags: vec![
                vec![Value::Int32(7), Value::Int32(8)],
                vec![],
                vec![Value::Int32(9)],
                vec![Value::Int32(10)],
            ],
            names: vec![
                vec![Value::Text("named.txt".into()), Value::Null],
                vec![Value::Text(String::new())],
                vec![],
                vec![],
            ],
        }
    }
}

impl Source for Mixed {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data.columns(table)
    }

    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }

    fn multivalue_columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.values)
    }

    fn collection_members(&self, _: &str) -> Result<&[CollectionMember], QueryError> {
        Ok(&self.members)
    }

    fn recordset_columns(&self, table: &str) -> Result<&[RecordsetColumn], QueryError> {
        self.data.recordset_columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.data.scan(table, visitor)
    }

    fn scan_multivalues(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.scan_collection_members(table, indices, values, &[], visitor)
    }

    fn scan_collection_members(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.scan_structured(
            table,
            indices,
            values,
            members,
            &[],
            &mut |row, lists, _| visitor(row, lists),
        )
    }

    fn scan_structured(
        &self,
        _: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        recordsets: &[usize],
        visitor: &mut StructuredVisitor<'_>,
    ) -> Result<(), QueryError> {
        assert!(values.is_empty() || values == [0]);
        assert!(members.is_empty() || members == [0]);
        assert!(recordsets.is_empty() || recordsets == [0]);
        for (parent, row) in self.data.rows.iter().enumerate() {
            // Independent source positions provide complete member lists, then
            // the same parent's immutable recordset; child expansion is query-owned.
            let lists = values
                .iter()
                .map(|_| &self.tags[parent] as &dyn MultivalueList)
                .chain(
                    members
                        .iter()
                        .map(|_| &self.names[parent] as &dyn MultivalueList),
                )
                .collect::<Vec<_>>();
            let records = recordsets
                .iter()
                .map(|_| Arc::clone(&self.data.records[parent]))
                .collect::<Vec<_>>();
            visitor(ProjectedRow::select(row, indices)?, &lists, &records)?;
        }
        Ok(())
    }
}

#[test]
fn whole_recordsets_lists_and_displayed_members_share_actual_parent_frames() {
    let source = Mixed::new();
    let result = super::run("SELECT Id,Files AS Children,Tags AS SavedTags,Files.FileName AS DisplayedName FROM T ORDER BY Id", &source).unwrap();
    assert_eq!(result.rows().len(), 5);
    let parents = [0, 0, 1, 2, 3];
    for (row, parent) in result.rows().iter().zip(parents) {
        assert_eq!(row[0], super::scalar(parent as i32 + 1));
        assert!(Arc::ptr_eq(
            super::recordset(&row[1]),
            &source.data.records[parent]
        ));
        let QueryValue::Multivalue(tags) = &row[2] else {
            panic!("expected complete Tags list")
        };
        assert_eq!(tags.values(), source.tags[parent]);
    }
    let displayed = [
        Value::Text("named.txt".into()),
        Value::Null,
        Value::Text(String::new()),
        Value::Null,
        Value::Null,
    ];
    for (row, value) in result.rows().iter().zip(displayed) {
        assert_eq!(row[3], QueryValue::Scalar(ExpressionValue::Value(value)));
    }
    assert!(result.recordset_schema(1).is_some());
    assert!(result.recordset_schema(2).is_none());
    assert!(result.recordset_schema(3).is_none());
}

#[test]
fn displayed_child_filter_does_not_filter_the_complete_owned_recordset() {
    let source = Mixed::new();
    let result = super::run("SELECT Id,Files,Tags,Files.FileName AS DisplayedName FROM T WHERE Files.FileName IS NOT NULL ORDER BY Id", &source).unwrap();
    assert_eq!(result.rows().len(), 2);
    assert_eq!(result.rows()[0][0], super::scalar(1));
    assert_eq!(result.rows()[1][0], super::scalar(2));
    let complete = super::recordset(&result.rows()[0][1]);
    assert!(Arc::ptr_eq(complete, &source.data.records[0]));
    assert_eq!(complete.len(), 2);
    // The NULL child is absent only from the displayed scalar rows; the complete
    // snapshot retains that child and its explicitly uninspected FileData.
    assert_eq!(
        complete.rows()[1],
        [StoredCell::Value(Value::Null), StoredCell::Uninspected]
    );
    let QueryValue::Multivalue(tags) = &result.rows()[0][2] else {
        panic!("expected complete Tags list")
    };
    assert_eq!(tags.values(), [Value::Int32(7), Value::Int32(8)]);
}
