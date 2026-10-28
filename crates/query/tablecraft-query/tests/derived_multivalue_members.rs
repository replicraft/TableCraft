use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Single, Value};
use tablecraft_db::{MultivalueList, ProjectedRow};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    CollectionQueryResult, ExecutionOptions, OutputType, QueryError, QueryValue, Source,
    execute_collections_with_schema_discovery, prepare, source::MultivalueVisitor,
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
            named: syntax("SELECT Id,Tags AS X,Tags AS Y FROM T"),
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
        Ok((name == "Bundle").then_some(&self.named))
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
fn repeated_aliases_keep_one_origin_across_named_and_inline_wrappers() {
    let data = Data::new();
    let result = execute(
        "SELECT Q.X,Q.X.Value,Q.Y.Value FROM (SELECT Id,X,Y FROM Bundle) Q ORDER BY Q.Id",
        &data,
    )
    .unwrap();
    let expected = [
        (0, Value::Null),
        (0, Value::Int32(10)),
        (0, Value::Int32(10)),
        (1, Value::Null),
        (2, Value::Int32(20)),
    ];
    assert_eq!(result.rows().len(), expected.len());
    for (row, (parent, child)) in result.rows().iter().zip(expected) {
        assert_eq!(list(&row[0]), data.lists[parent][0]);
        assert_eq!(row[1], scalar(child.clone()));
        assert_eq!(row[2], scalar(child));
    }
    assert_eq!(data.scans.get(), 1);
}

#[test]
fn independent_named_relation_instances_expand_independently_even_with_equal_payloads() {
    let data = Data::new();
    let result = execute(
        "SELECT Q.X.Value,Q.Y.Value FROM (SELECT L.X AS X,R.X AS Y FROM Bundle L,Bundle R) Q",
        &data,
    )
    .unwrap();
    let mut expected = Vec::new();
    for left in &data.lists {
        for right in &data.lists {
            let left = if left[0].is_empty() {
                vec![Value::Null]
            } else {
                left[0].clone()
            };
            let right = if right[0].is_empty() {
                vec![Value::Null]
            } else {
                right[0].clone()
            };
            for l in &left {
                for r in &right {
                    expected.push(vec![scalar(l.clone()), scalar(r.clone())]);
                }
            }
        }
    }
    assert_eq!(result.rows(), expected);
    assert_eq!(data.scans.get(), 2);
}

#[test]
fn alias_lineage_controls_hidden_atoms_and_scalar_api_guards_without_row_inference() {
    let mut data = Data::new();
    data.lists[0][0] = vec![Value::Int32(1), Value::Int32(2)];
    let result = execute(
        "SELECT Q.X,Q.X.Value FROM Bundle Q WHERE Q.Y.Value=2",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 1);
    assert_eq!(list(&result.rows()[0][0]), data.lists[0][0]);
    assert_eq!(result.rows()[0][1], scalar(Value::Int32(2)));
    let result = execute("SELECT Q.X FROM Bundle Q WHERE Q.X.Value<>Q.Y.Value", &data).unwrap();
    // Hidden <> retains the established NOT Any(Equal) contract, including
    // true for an empty present collection. Same-origin nonempty aliases compare
    // their aligned items, rather than finding unequal cross-product items.
    assert_eq!(result.rows().len(), 1);
    assert!(list(&result.rows()[0][0]).is_empty());
    let result = execute("SELECT Q.X FROM Bundle Q WHERE Q.X.Value>Q.Y.Value", &data).unwrap();
    assert!(result.rows().is_empty());
    let result = execute(
        "SELECT Q.X FROM Bundle Q WHERE Q.X.Value=1 AND Q.Y.Value=2",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 1);
    assert_eq!(list(&result.rows()[0][0]), data.lists[0][0]);

    let data = Data::new();
    let plan = prepare(&syntax("SELECT Q.X.Value FROM Bundle Q"), &data).unwrap();
    assert_eq!(data.scans.get(), 0);
    assert!(
        matches!(plan.output_types().unwrap()[0], OutputType::Scalar { data_type, nullable: true }
        if data_type.scalar_type() == ScalarType::Int32)
    );
    assert_eq!(
        plan.execute(&data, &[], Default::default()).err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(
        plan.execute_expression(&data, &[], Default::default())
            .err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(data.scans.get(), 0);
    assert_eq!(
        plan.execute_collections(&data, &[], Default::default())
            .unwrap()
            .rows()
            .len(),
        5
    );
}

#[test]
fn retained_whole_ieee_data_survives_independent_member_expansion_but_not_scalar_use() {
    let mut data = Data::new();
    data.elements[0] = Column::new("Tags".into(), ScalarType::Single, true).unwrap();
    let bits = [0, 0x80000000, 1, 0x7f800000, 0x7fc00011, 0xffc00021];
    for row in &mut data.lists {
        row[0] = bits
            .map(|bits| Value::Single(Single::from_bits(bits)))
            .to_vec();
    }
    let result = execute(
        "SELECT Q.X,Q.Other.Value FROM (SELECT Tags AS X,Other FROM T) Q",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 5);
    for row in result.rows() {
        let actual = list(&row[0])
            .iter()
            .map(|value| {
                let Value::Single(value) = value else {
                    panic!("stored Single became another carrier");
                };
                value.to_bits()
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, bits);
    }
    assert_eq!(
        execute("SELECT Q.X.Value FROM (SELECT Tags AS X FROM T) Q", &data).err(),
        Some(QueryError::Evaluation(EvalError::NonFinite))
    );
}
