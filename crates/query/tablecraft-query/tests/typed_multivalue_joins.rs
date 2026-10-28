use std::cell::{Cell, RefCell};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_db::{MultivalueList, ProjectedRow};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    CollectionQueryResult, ExecutionOptions, QueryError, QueryValue, Source,
    execute_collections_with_schema_discovery, source::MultivalueVisitor,
};
use tablecraft_schema::{Column, ScalarType, SchemaName};
use tablecraft_sql::{parse_query, query::Query};

#[path = "typed_multivalue_joins/borrowed.rs"]
mod borrowed;
#[path = "typed_multivalue_joins/scalar_aggregate.rs"]
mod scalar_aggregate;

struct Data {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    elements: Vec<Column>,
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
fn duplicate_inner_matches_keep_both_complete_lists_in_pair_encounter_order() {
    let data = Data::new();
    let result = execute(
        "SELECT L.Seq,R.Seq,L.Tags,R.Tags FROM LeftRows L INNER JOIN RightRows R ON L.Id=R.Id",
        &data,
    )
    .unwrap();
    let pairs = [(0, 0), (0, 1), (1, 0), (1, 1)];
    assert_eq!(result.rows().len(), pairs.len());
    for (row, (left, right)) in result.rows().iter().zip(pairs) {
        assert_eq!(integer(&row[0]), left as i32);
        assert_eq!(integer(&row[1]), right as i32);
        assert_eq!(list(&row[2]), data.left_lists[left]);
        assert_eq!(list(&row[3]), data.right_lists[right]);
    }
    assert_eq!(*data.scans.borrow(), ["lists:LeftRows", "lists:RightRows"]);
}

#[test]
fn cross_named_derived_and_scalar_leaves_compose_without_extra_child_scans() {
    let data = Data::new();
    let result = execute(
        "SELECT TOP 2 L.Tags,R.Tags,L.Seq,R.Seq FROM PairView L,RightRows R ORDER BY L.Seq,R.Seq",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 2);
    assert_eq!(list(&result.rows()[0][0]), data.left_lists[0]);
    assert_eq!(list(&result.rows()[0][1]), data.right_lists[0]);
    assert_eq!(list(&result.rows()[1][1]), data.right_lists[1]);
    data.scans.borrow_mut().clear();
    let result = execute("SELECT L.Tags,P.Id FROM (SELECT Id,Tags FROM LeftRows) L INNER JOIN Plain P ON L.Id=P.Id WHERE P.Seq=0", &data).unwrap();
    assert_eq!(result.rows().len(), 2);
    assert_eq!(*data.scans.borrow(), ["lists:LeftRows", "scalar:Plain"]);
}

#[test]
fn joined_hidden_criteria_select_parents_without_trimming_either_list() {
    let data = Data::new();
    let result = execute("SELECT L.Tags,R.Tags FROM LeftRows L INNER JOIN RightRows R ON L.Id=R.Id WHERE L.Tags.Value=10 AND R.Tags.Value=101", &data).unwrap();
    assert_eq!(result.rows().len(), 1);
    assert_eq!(list(&result.rows()[0][0]), data.left_lists[0]);
    assert_eq!(list(&result.rows()[0][1]), data.right_lists[1]);
}

#[test]
fn source_and_eager_on_failures_never_publish_a_join_prefix() {
    let mut data = Data::new();
    let error = execute("SELECT L.Tags FROM LeftRows L INNER JOIN RightRows R ON False AND 1/(L.Id-L.Id)=0 WHERE False", &data).err();
    assert_eq!(
        error,
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(*data.scans.borrow(), ["lists:LeftRows", "scalar:RightRows"]);
    data.left.clear();
    data.left_lists.clear();
    data.right_lists[2].push(Value::Text("invalid selected suffix".into()));
    assert_eq!(
        execute("SELECT R.Tags FROM LeftRows L,RightRows R", &data).err(),
        Some(QueryError::Source)
    );
    data.right_lists[2].pop();
    data.fail_right_eof = true;
    assert_eq!(
        execute("SELECT R.Tags FROM LeftRows L,RightRows R", &data).err(),
        Some(QueryError::Source)
    );
}
