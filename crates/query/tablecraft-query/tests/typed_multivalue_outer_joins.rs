use std::cell::{Cell, RefCell};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_db::{MultivalueList, ProjectedRow};
use tablecraft_query::{
    CollectionQueryResult, ExecutionOptions, OutputType, QueryError, QueryValue, Source,
    execute_collections_with_schema_discovery, source::MultivalueVisitor,
};
use tablecraft_schema::{CollectionMember, Column, ScalarType, SchemaName};
use tablecraft_sql::{parse_query, query::Query};

struct Data {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    elements: Vec<Column>,
    members: Vec<CollectionMember>,
    left_members: Vec<Vec<Value>>,
    right_members: Vec<Vec<Value>>,
    left: Vec<Vec<Value>>,
    right: Vec<Vec<Value>>,
    left_lists: Vec<Vec<Value>>,
    right_lists: Vec<Vec<Value>>,
    named: Query,
    scans: RefCell<Vec<String>>,
    fail_right_eof: bool,
    ignore_failure: bool,
    callbacks: Cell<usize>,
}

impl Data {
    fn new() -> Self {
        Self {
            columns: ["Id", "Seq"]
                .map(|name| Column::new(name.into(), ScalarType::Int32, false).unwrap())
                .to_vec(),
            unavailable: vec![SchemaName::new("Tags".into()).unwrap()],
            elements: vec![Column::new("Tags".into(), ScalarType::Int32, true).unwrap()],
            members: Vec::new(),
            left_members: Vec::new(),
            right_members: Vec::new(),
            left: vec![
                vec![Value::Int32(1), Value::Int32(0)],
                vec![Value::Int32(1), Value::Int32(1)],
                vec![Value::Int32(2), Value::Int32(2)],
            ],
            right: vec![
                vec![Value::Int32(1), Value::Int32(0)],
                vec![Value::Int32(1), Value::Int32(1)],
                vec![Value::Int32(3), Value::Int32(2)],
            ],
            left_lists: vec![
                vec![Value::Null, Value::Int32(10), Value::Int32(10)],
                vec![],
                vec![Value::Int32(20)],
            ],
            right_lists: vec![
                vec![Value::Int32(100)],
                vec![Value::Int32(101), Value::Int32(101)],
                vec![Value::Int32(300)],
            ],
            named: parse_query("SELECT Id,Seq,Tags FROM LeftRows", Default::default())
                .unwrap()
                .query()
                .clone(),
            scans: RefCell::new(Vec::new()),
            fail_right_eof: false,
            ignore_failure: false,
            callbacks: Cell::new(0),
        }
    }

    fn rows(&self, table: &str) -> Result<&[Vec<Value>], QueryError> {
        match table {
            "LeftRows" | "Plain" => Ok(&self.left),
            "RightRows" => Ok(&self.right),
            _ => Err(QueryError::UnknownName),
        }
    }
}

impl Source for Data {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.rows(table)?;
        Ok(&self.columns)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "PairView").then_some(&self.named))
    }

    fn unavailable_columns(&self, table: &str) -> Result<&[SchemaName], QueryError> {
        self.rows(table)?;
        Ok(if table == "Plain" {
            &[]
        } else {
            &self.unavailable
        })
    }

    fn multivalue_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.rows(table)?;
        Ok(if table == "Plain" {
            &[]
        } else {
            &self.elements
        })
    }

    fn collection_members(&self, table: &str) -> Result<&[CollectionMember], QueryError> {
        self.rows(table)?;
        Ok(if table == "Plain" { &[] } else { &self.members })
    }

    fn scan_collection_members(
        &self,
        table: &str,
        scalars: &[usize],
        collections: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        if members.is_empty() {
            return self.scan_multivalues(table, scalars, collections, visitor);
        }
        self.scans.borrow_mut().push(format!("members:{table}"));
        let (lists, named) = if table == "LeftRows" {
            (&self.left_lists, &self.left_members)
        } else {
            (&self.right_lists, &self.right_members)
        };
        for (position, row) in self.rows(table)?.iter().enumerate() {
            let mut views = Vec::new();
            for index in collections {
                assert_eq!(*index, 0);
                views.push(&lists[position] as &dyn MultivalueList);
            }
            for index in members {
                assert_eq!(*index, 0);
                views.push(&named[position] as &dyn MultivalueList);
            }
            visitor(ProjectedRow::select(row, scalars)?, &views)?;
        }
        Ok(())
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.borrow_mut().push(format!("scalar:{table}"));
        for row in self.rows(table)? {
            visitor(row)?;
        }
        Ok(())
    }

    fn scan_multivalues(
        &self,
        table: &str,
        scalars: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        assert_ne!(
            table, "Plain",
            "scalar leaves must use the ordinary Source service"
        );
        self.scans.borrow_mut().push(format!("lists:{table}"));
        let lists = if table == "LeftRows" {
            &self.left_lists
        } else {
            &self.right_lists
        };
        for (row, values) in self.rows(table)?.iter().zip(lists) {
            let views = collections
                .iter()
                .map(|index| {
                    assert_eq!(*index, 0);
                    values as &dyn MultivalueList
                })
                .collect::<Vec<_>>();
            self.callbacks.set(self.callbacks.get() + 1);
            let result = visitor(ProjectedRow::select(row, scalars)?, &views);
            if !self.ignore_failure {
                result?;
            }
        }
        if table == "RightRows" && self.fail_right_eof {
            Err(QueryError::Source)
        } else {
            Ok(())
        }
    }
}

fn execute(sql: &str, source: &Data) -> Result<CollectionQueryResult, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    execute_collections_with_schema_discovery(
        query.query(),
        source,
        &[],
        ExecutionOptions::default(),
    )
}

fn list(value: &QueryValue) -> &[Value] {
    match value {
        QueryValue::Multivalue(value) => value.values(),
        QueryValue::Scalar(_) | QueryValue::AbsentMultivalue => {
            panic!("expected a complete present list")
        }
    }
}

fn integer(value: &QueryValue) -> i32 {
    match value {
        QueryValue::Scalar(ExpressionValue::Value(Value::Int32(value))) => *value,
        _ => panic!("expected Int32"),
    }
}

#[test]
fn left_and_right_results_distinguish_present_empty_null_children_and_absence() {
    let mut data = Data::new();
    data.right_lists[2].clear();
    for (kind, absent_side) in [("LEFT", 3), ("RIGHT", 2)] {
        let result = execute(
            &format!("SELECT L.Seq,R.Seq,L.Tags,R.Tags FROM LeftRows L {kind} JOIN RightRows R ON L.Id=R.Id"),
            &data,
        ).unwrap();
        assert_eq!(result.rows().len(), 5);
        let pairs = [(0, 0), (0, 1), (1, 0), (1, 1)];
        for (row, (left, right)) in result.rows().iter().zip(pairs) {
            assert_eq!(integer(&row[0]), left as i32);
            assert_eq!(integer(&row[1]), right as i32);
            assert_eq!(list(&row[2]), data.left_lists[left]);
            assert_eq!(list(&row[3]), data.right_lists[right]);
        }
        let last = &result.rows()[4];
        assert_eq!(last[absent_side], QueryValue::AbsentMultivalue);
        if kind == "LEFT" {
            assert_eq!(
                last[1],
                QueryValue::Scalar(ExpressionValue::Value(Value::Null))
            );
            assert_eq!(list(&last[2]), data.left_lists[2]);
        } else {
            assert_eq!(
                last[0],
                QueryValue::Scalar(ExpressionValue::Value(Value::Null))
            );
            assert!(list(&last[3]).is_empty());
        }
        for (index, nullable) in [(2, kind == "RIGHT"), (3, kind == "LEFT")] {
            assert!(
                matches!(result.output_types()[index], OutputType::Multivalue {
                element_nullable: true, parent_nullable, ..
            } if parent_nullable == nullable)
            );
        }
    }
}

#[test]
fn derived_repeated_outputs_keep_missing_parent_metadata_and_presence() {
    let data = Data::new();
    let result = execute(
        "SELECT X.B,X.A,X.B FROM (SELECT L.Tags AS A,R.Tags AS B,L.Seq AS N FROM LeftRows L LEFT JOIN RightRows R ON L.Id=R.Id) X ORDER BY X.N",
        &data,
    ).unwrap();
    assert_eq!(result.rows().len(), 5);
    let last = &result.rows()[4];
    assert_eq!(last[0], QueryValue::AbsentMultivalue);
    assert_eq!(last[2], QueryValue::AbsentMultivalue);
    assert_eq!(list(&last[1]), data.left_lists[2]);
    for index in [0, 2] {
        assert!(matches!(
            result.output_types()[index],
            OutputType::Multivalue {
                element_nullable: true,
                parent_nullable: true,
                ..
            }
        ));
    }
}

#[test]
fn hidden_any_on_absence_is_unknown_and_eager_input_failures_stay_unconditional() {
    let data = Data::new();
    let result = execute(
        "SELECT L.Seq,R.Tags FROM LeftRows L LEFT JOIN RightRows R ON L.Id=R.Id WHERE NOT (R.Tags.Value=100)",
        &data,
    ).unwrap();
    assert_eq!(result.rows().len(), 2);
    for row in result.rows() {
        assert_eq!(list(&row[1]), data.right_lists[1]);
    }
    let mut data = Data::new();
    data.left.clear();
    data.right_lists[2].push(Value::Text("invalid stored Int32".into()));
    assert_eq!(
        execute(
            "SELECT R.Tags FROM LeftRows L LEFT JOIN RightRows R ON False WHERE False",
            &data
        )
        .err(),
        Some(QueryError::Source)
    );
}

#[test]
fn hidden_named_collection_presence_distinguishes_absent_empty_and_nonempty_parents() {
    let mut data = Data::new();
    data.unavailable
        .push(SchemaName::new("Files".into()).unwrap());
    data.members.push(
        CollectionMember::new(
            "Files".into(),
            Column::new("FileName".into(), ScalarType::Text, true).unwrap(),
        )
        .unwrap(),
    );
    data.left_members = vec![vec![Value::Text("left".into())], vec![], vec![]];
    data.right_members = vec![vec![Value::Text("right".into())], vec![], vec![Value::Null]];

    // Presence observes the named container, independently of the projected
    // scalar-valued Tags list. None is a missing parent, not a stored child NULL.
    for (kind, predicate, pairs) in [
        (
            "LEFT",
            "R.Files IS NULL",
            vec![(Some(0), Some(1)), (Some(1), Some(1)), (Some(2), None)],
        ),
        (
            "LEFT",
            "R.Files IS NOT NULL",
            vec![(Some(0), Some(0)), (Some(1), Some(0))],
        ),
        (
            "RIGHT",
            "L.Files IS NULL",
            vec![(Some(1), Some(0)), (Some(1), Some(1)), (None, Some(2))],
        ),
        (
            "RIGHT",
            "L.Files IS NOT NULL",
            vec![(Some(0), Some(0)), (Some(0), Some(1))],
        ),
        (
            "RIGHT",
            "R.Files IS NOT NULL",
            vec![(Some(0), Some(0)), (Some(1), Some(0)), (None, Some(2))],
        ),
    ] {
        let sql = format!(
            "SELECT L.Seq,R.Seq,L.Tags,R.Tags FROM LeftRows L {kind} JOIN RightRows R ON L.Id=R.Id WHERE {predicate}"
        );
        let result = execute(&sql, &data).unwrap();
        assert_eq!(result.rows().len(), pairs.len(), "{sql}");
        for (row, (left, right)) in result.rows().iter().zip(pairs) {
            for (side, position, lists) in
                [(0, left, &data.left_lists), (1, right, &data.right_lists)]
            {
                if let Some(position) = position {
                    assert_eq!(integer(&row[side]), position as i32, "{sql}");
                    assert_eq!(list(&row[side + 2]), lists[position], "{sql}");
                } else {
                    assert_eq!(
                        row[side],
                        QueryValue::Scalar(ExpressionValue::Value(Value::Null)),
                        "{sql}"
                    );
                    assert_eq!(row[side + 2], QueryValue::AbsentMultivalue, "{sql}");
                }
            }
        }
    }
}
