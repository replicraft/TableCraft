use proptest::prelude::*;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Bucket".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                vec![Value::Int32(1), Value::Int32(1), Value::Text("ab".into())],
                vec![Value::Int32(2), Value::Int32(1), Value::Text("cd".into())],
                vec![Value::Int32(3), Value::Int32(2), Value::Null],
                vec![Value::Int32(4), Value::Int32(2), Value::Text("e".into())],
                vec![Value::Int32(5), Value::Int32(3), Value::Text("f".into())],
                vec![Value::Int32(6), Value::Int32(3), Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &MemorySource) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
#[test]
fn computed_group_keys_work_in_projection_having_and_sorting_after_pruning() {
    let source = source();
    assert_eq!(
        run(
            "SELECT Len(Label) AS Width,Count(*) FROM Items GROUP BY Len(Label) ORDER BY Width",
            &source
        )
        .unwrap(),
        [
            vec![Value::Null, Value::Int64(2)],
            vec![Value::Int32(1), Value::Int64(2)],
            vec![Value::Int32(2), Value::Int64(2)],
        ]
    );
    assert_eq!(run("SELECT Len(i.Label)+1,Count(*) FROM Items AS i GROUP BY Len(Label) HAVING Len(i.Label)=2 ORDER BY Len(Label)",&source).unwrap(),[vec![Value::Int64(3),Value::Int64(2)]]);
    assert_eq!(
        run(
            "SELECT Bucket Mod 2 AS K,Sum(Id) FROM Items GROUP BY Bucket Mod 2 ORDER BY K",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int64(0), Value::Int64(7)],
            vec![Value::Int64(1), Value::Int64(14)],
        ]
    );
}
#[test]
fn structural_group_membership_does_not_admit_hidden_ungrouped_fields() {
    let source = source();
    for sql in [
        "SELECT Label FROM Items GROUP BY Len(Label)",
        "SELECT Id+Bucket FROM Items GROUP BY Bucket+Id",
        "SELECT Len(Label),Id FROM Items GROUP BY Len(Label)",
        "SELECT Count(*) FROM Items GROUP BY Len(Label) HAVING Label IS NULL",
    ] {
        assert!(
            matches!(run(sql, &source), Err(QueryError::Grouping)),
            "{sql}"
        );
    }
    assert!(matches!(
        run("SELECT Count(*) FROM Items GROUP BY (SELECT 1)", &source),
        Err(QueryError::Unsupported)
    ));
}
#[test]
fn first_last_keep_encounter_order_nulls_and_full_values() {
    let source = source();
    assert_eq!(
        run(
            "SELECT Bucket,First(Label),Last(Label) FROM Items GROUP BY Bucket ORDER BY Bucket",
            &source
        )
        .unwrap(),
        [
            vec![
                Value::Int32(1),
                Value::Text("ab".into()),
                Value::Text("cd".into())
            ],
            vec![Value::Int32(2), Value::Null, Value::Text("e".into())],
            vec![Value::Int32(3), Value::Text("f".into()), Value::Null],
        ]
    );
    assert_eq!(
        run(
            "SELECT First(Id),Last(Id) FROM (SELECT TOP 6 Id FROM Items ORDER BY Id DESC) AS R",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(6), Value::Int32(1)]]
    );
    assert_eq!(
        run(
            "SELECT First(Label),Last(Label),Count(*) FROM Items WHERE Id<0",
            &source
        )
        .unwrap(),
        [vec![Value::Null, Value::Null, Value::Int64(0)]]
    );
}
#[test]
fn encounter_aggregates_do_not_hide_later_errors_or_accept_invalid_calls() {
    let source = source();
    for function in ["First", "Last"] {
        assert!(matches!(
            run(
                &format!("SELECT {function}(10 \\ (Id-2)) FROM Items"),
                &source
            ),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        ));
        for expression in [
            format!("{function}(*)"),
            format!("{function}(Id,Id)"),
            format!("{function}(Sum(Id))"),
        ] {
            assert!(matches!(
                run(&format!("SELECT {expression} FROM Items"), &source),
                Err(QueryError::Unsupported)
            ));
        }
    }
}
proptest! {
    #[test]
    fn arithmetic_groups_and_encounter_endpoints_match_an_independent_partition(values in prop::collection::vec(-100_i32..100,0..48)) {
        let rows=values.iter().map(|v|vec![Value::Int32(*v)]).collect();
        let source=MemorySource::new(vec![MemoryTable::new("Items".into(),vec![Column::new("N".into(),ScalarType::Int32,false).unwrap()],rows).unwrap()]).unwrap();
        let actual=run("SELECT N Mod 3 AS K,Count(*),First(N),Last(N) FROM Items GROUP BY N Mod 3 ORDER BY K",&source).unwrap();
        let mut groups=std::collections::BTreeMap::<i32,Vec<i32>>::new();
        for value in values {groups.entry(value%3).or_default().push(value);}
        let expected=groups.into_iter().map(|(key,rows)|vec![Value::Int64(i64::from(key)),Value::Int64(rows.len() as i64),Value::Int32(rows[0]),Value::Int32(*rows.last().unwrap())]).collect::<Vec<_>>();
        prop_assert_eq!(actual,expected);
    }
}
