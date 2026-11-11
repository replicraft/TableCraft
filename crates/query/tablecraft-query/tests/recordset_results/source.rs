use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    MemberAvailability, OwnedRecordset, ProjectedRow, RecordsetPresence, RecordsetSchema,
    StoredCell,
};
use tablecraft_query::{
    QueryError, Source,
    source::{RecordsetColumn, StructuredVisitor},
};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaName};

pub struct ScanRequest {
    pub table: String,
    pub scalar_indices: Vec<usize>,
    pub recordset_indices: Vec<usize>,
}

pub struct Data {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    declarations: Vec<Vec<RecordsetColumn>>,
    pub rows: Vec<Vec<Value>>,
    pub records: Vec<Arc<OwnedRecordset>>,
    right_rows: Vec<Vec<Value>>,
    right_records: Vec<Arc<OwnedRecordset>>,
    pub changed: Cell<usize>,
    pub scans: Cell<usize>,
    pub requests: RefCell<Vec<ScanRequest>>,
    pub fail_suffix: Cell<bool>,
    pub wrong_schema: Cell<bool>,
    pub empty_callback: Cell<bool>,
}

fn schema(change: usize) -> Arc<RecordsetSchema> {
    Arc::new(
        RecordsetSchema::new(
            vec![
                Column::new(
                    if change == 1 { "Changed" } else { "FileName" }.into(),
                    if change == 2 {
                        ScalarType::Binary
                    } else {
                        ScalarType::Text
                    },
                    true,
                )
                .unwrap(),
                Column::new("FileData".into(), ScalarType::Binary, true).unwrap(),
            ],
            vec![
                if change == 3 {
                    MemberAvailability::Uninspected
                } else {
                    MemberAvailability::Available
                },
                MemberAvailability::Uninspected,
            ],
            if change == 4 {
                NameComparison::Exact
            } else {
                NameComparison::AsciiInsensitive
            },
            &mut ResourceBudget::new(0, 0, 1000),
            &|| false,
        )
        .unwrap(),
    )
}

fn record(
    schema: &Arc<RecordsetSchema>,
    values: &[Option<&str>],
    presence: RecordsetPresence,
) -> Arc<OwnedRecordset> {
    let rows = values
        .iter()
        .map(|value| {
            vec![
                StoredCell::Value(value.map_or(Value::Null, |value| Value::Text(value.into()))),
                StoredCell::Uninspected,
            ]
        })
        .collect();
    Arc::new(
        OwnedRecordset::new(
            Arc::clone(schema),
            rows,
            presence,
            &mut ResourceBudget::new(0, 0, 1000),
            &|| false,
        )
        .unwrap(),
    )
}

impl Data {
    pub fn new() -> Self {
        let original = schema(0);
        let records = vec![
            record(
                &original,
                &[Some("named.txt"), None],
                RecordsetPresence::Assigned,
            ),
            record(&original, &[Some("")], RecordsetPresence::Assigned),
            record(&original, &[], RecordsetPresence::Assigned),
            record(&original, &[], RecordsetPresence::Unassigned),
        ];
        Self {
            columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            unavailable: vec![SchemaName::new("Files".into()).unwrap()],
            declarations: (0..6)
                .map(|change| {
                    vec![
                        RecordsetColumn::new(
                            "Files".into(),
                            if change == 0 {
                                Arc::clone(&original)
                            } else {
                                schema(change)
                            },
                        )
                        .unwrap(),
                    ]
                })
                .collect(),
            rows: (1..=4).map(|id| vec![Value::Int32(id)]).collect(),
            records,
            right_rows: [2, 5].map(|id| vec![Value::Int32(id)]).into(),
            right_records: vec![
                record(&original, &[Some("right-two")], RecordsetPresence::Assigned),
                record(
                    &original,
                    &[Some("right-five")],
                    RecordsetPresence::Assigned,
                ),
            ],
            changed: Cell::new(0),
            scans: Cell::new(0),
            requests: RefCell::new(Vec::new()),
            fail_suffix: Cell::new(false),
            wrong_schema: Cell::new(false),
            empty_callback: Cell::new(false),
        }
    }

    fn table(&self, table: &str) -> (&[Vec<Value>], &[Arc<OwnedRecordset>]) {
        if table.eq_ignore_ascii_case("U") {
            (&self.right_rows, &self.right_records)
        } else {
            (&self.rows, &self.records)
        }
    }
}

impl Source for Data {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }

    fn recordset_columns(&self, _: &str) -> Result<&[RecordsetColumn], QueryError> {
        Ok(&self.declarations[self.changed.get()])
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in self.table(table).0 {
            visitor(row)?;
        }
        Ok(())
    }

    fn scan_structured(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        recordsets: &[usize],
        visitor: &mut StructuredVisitor<'_>,
    ) -> Result<(), QueryError> {
        assert!(values.is_empty() && members.is_empty());
        assert!(recordsets.is_empty() || recordsets == [0]);
        self.scans.set(self.scans.get() + 1);
        self.requests.borrow_mut().push(ScanRequest {
            table: table.into(),
            scalar_indices: indices.into(),
            recordset_indices: recordsets.into(),
        });
        let (rows, records) = self.table(table);
        for (row, recordset) in rows.iter().zip(records) {
            let selected = if self.empty_callback.get() || recordsets.is_empty() {
                Vec::new()
            } else if self.wrong_schema.get() {
                vec![record(
                    self.declarations[1][0].schema(),
                    &[Some("wrong")],
                    RecordsetPresence::Assigned,
                )]
            } else {
                vec![Arc::clone(recordset)]
            };
            visitor(ProjectedRow::select(row, indices)?, &[], &selected)?;
        }
        if self.fail_suffix.get() {
            Err(QueryError::Source)
        } else {
            Ok(())
        }
    }
}
