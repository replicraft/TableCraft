use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_query::{
    CollectionQueryResult, ExecutionOptions, QueryError, QueryValue, Source,
    execute_collections_with_schema_discovery,
    source::{MultivalueList, MultivalueVisitor, ProjectedRow},
};
use tablecraft_schema::{CollectionMember, Column, ScalarType, SchemaName};
use tablecraft_sql::{parse_query, query::Query};

struct Data {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    elements: Vec<Column>,
    members: Vec<CollectionMember>,
    ids: Vec<Vec<Value>>,
    lists: Vec<Vec<Vec<Value>>>,
    other_ids: Vec<Vec<Value>>,
    other_lists: Vec<Vec<Vec<Value>>>,
    named: Query,
    scans: Cell<usize>,
}

impl Data {
    fn new() -> Self {
        Self {
            columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            unavailable: ["Tags", "Other", "Files"]
                .map(|name| SchemaName::new(name.into()).unwrap())
                .to_vec(),
            elements: ["Tags", "Other"]
                .map(|name| Column::new(name.into(), ScalarType::Int32, true).unwrap())
                .to_vec(),
            members: ["Name", "Kind"]
                .map(|name| {
                    CollectionMember::new(
                        "Files".into(),
                        Column::new(name.into(), ScalarType::Text, true).unwrap(),
                    )
                    .unwrap()
                })
                .to_vec(),
            ids: [1, 2, 3].map(|id| vec![Value::Int32(id)]).to_vec(),
            lists: vec![
                vec![
                    vec![Value::Null, Value::Int32(10), Value::Int32(10)],
                    vec![Value::Int32(1), Value::Int32(2)],
                    vec![text("a"), text("b")],
                    vec![text("txt"), text("png")],
                ],
                vec![vec![], vec![Value::Int32(3)], vec![], vec![]],
                vec![
                    vec![Value::Int32(20)],
                    vec![Value::Int32(4), Value::Int32(5)],
                    vec![Value::Null],
                    vec![Value::Null],
                ],
            ],
            other_ids: [1, 4].map(|id| vec![Value::Int32(id)]).to_vec(),
            other_lists: vec![
                vec![
                    vec![Value::Int32(7), Value::Int32(8)],
                    vec![],
                    vec![],
                    vec![],
                ],
                vec![vec![], vec![], vec![], vec![]],
            ],
            named: syntax("SELECT Id,Tags AS Whole,Tags.Value AS One FROM T"),
            scans: Cell::new(0),
        }
    }
}

impl Source for Data {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if matches!(table, "T" | "U") {
            Ok(&self.columns)
        } else {
            Err(QueryError::UnknownName)
        }
    }

    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }

    fn multivalue_columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.elements)
    }

    fn collection_members(&self, _: &str) -> Result<&[CollectionMember], QueryError> {
        Ok(&self.members)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "MixedRows").then_some(&self.named))
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("selected lists and members must share their collection cursor")
    }

    fn scan_collection_members(
        &self,
        table: &str,
        scalars: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        let (ids, lists) = match table {
            "T" => (&self.ids, &self.lists),
            "U" => (&self.other_ids, &self.other_lists),
            _ => return Err(QueryError::UnknownName),
        };
        for (row, lists) in ids.iter().zip(lists) {
            let views = values
                .iter()
                .copied()
                .chain(members.iter().map(|member| member + 2))
                .map(|index| &lists[index] as &dyn MultivalueList)
                .collect::<Vec<_>>();
            visitor(ProjectedRow::select(row, scalars)?, &views)?;
        }
        Ok(())
    }
}

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

fn execute(sql: &str, source: &Data) -> Result<CollectionQueryResult, QueryError> {
    execute_collections_with_schema_discovery(
        &syntax(sql),
        source,
        &[],
        ExecutionOptions::default(),
    )
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

fn scalar(value: Value) -> QueryValue {
    QueryValue::Scalar(ExpressionValue::Value(value))
}

fn list(value: &QueryValue) -> &[Value] {
    let QueryValue::Multivalue(value) = value else {
        panic!("expected a complete present list");
    };
    value.values()
}

#[test]
fn complete_lists_repeat_for_each_displayed_child_and_empty_placeholder() {
    let data = Data::new();
    let result = execute("SELECT Id,Tags,Tags.Value FROM T ORDER BY Id", &data).unwrap();
    let expected = [
        (0, Value::Null),
        (0, Value::Int32(10)),
        (0, Value::Int32(10)),
        (1, Value::Null),
        (2, Value::Int32(20)),
    ];
    assert_eq!(result.rows().len(), expected.len());
    for (row, (parent, child)) in result.rows().iter().zip(expected) {
        assert_eq!(row[0], scalar(data.ids[parent][0].clone()));
        assert_eq!(list(&row[1]), data.lists[parent][0]);
        assert_eq!(row[2], scalar(child));
    }
    assert_eq!(data.scans.get(), 1);
}

#[test]
fn current_displayed_predicates_and_independent_hidden_membership_keep_full_lists() {
    let data = Data::new();
    let result = execute(
        "SELECT Tags,Tags.Value FROM T WHERE Other.Value=2 AND Tags.Value=10",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 2);
    for row in result.rows() {
        assert_eq!(list(&row[0]), data.lists[0][0]);
        assert_eq!(row[1], scalar(Value::Int32(10)));
    }
    let result = execute("SELECT Tags,Tags.Value,Other.Value FROM T", &data).unwrap();
    assert_eq!(result.rows().len(), 9);
    let result = execute("SELECT Tags,Files.Name,Files.Kind FROM T", &data).unwrap();
    assert_eq!(result.rows().len(), 4);
    for (row, (parent, name, kind)) in result.rows().iter().zip([
        (0, text("a"), text("txt")),
        (0, text("b"), text("png")),
        (1, Value::Null, Value::Null),
        (2, Value::Null, Value::Null),
    ]) {
        assert_eq!(list(&row[0]), data.lists[parent][0]);
        assert_eq!(row[1], scalar(name));
        assert_eq!(row[2], scalar(kind));
    }
}

#[test]
fn named_derived_and_outer_joins_preserve_whole_list_presence_with_scalar_members() {
    let data = Data::new();
    let result = execute(
        "SELECT TOP 2 One,Whole FROM (SELECT Id,Whole,One FROM MixedRows) Q ORDER BY Id DESC",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 2);
    assert_eq!(result.rows()[0][0], scalar(Value::Int32(20)));
    assert_eq!(list(&result.rows()[0][1]), data.lists[2][0]);
    assert_eq!(result.rows()[1][0], scalar(Value::Null));
    assert!(list(&result.rows()[1][1]).is_empty());

    let result = execute(
        "SELECT L.Tags,R.Tags,R.Tags.Value FROM T L LEFT JOIN U R ON L.Id=R.Id",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 4);
    for (row, child) in result.rows()[..2].iter().zip([7, 8]) {
        assert_eq!(list(&row[0]), data.lists[0][0]);
        assert_eq!(list(&row[1]), data.other_lists[0][0]);
        assert_eq!(row[2], scalar(Value::Int32(child)));
    }
    for (row, parent) in result.rows()[2..].iter().zip([1, 2]) {
        assert_eq!(list(&row[0]), data.lists[parent][0]);
        assert_eq!(row[1], QueryValue::AbsentMultivalue);
        assert_eq!(row[2], scalar(Value::Null));
    }
}
