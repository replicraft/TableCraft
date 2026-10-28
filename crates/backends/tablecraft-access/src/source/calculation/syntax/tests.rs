use super::prepare;
use std::cmp::Ordering;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, prepare as bind,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn ordinary_text_dependencies_are_deduplicated_and_concatenation_executes() {
    let columns = vec![
        Column::new("FirstName".into(), ScalarType::Text, true).unwrap(),
        Column::new("LastName".into(), ScalarType::Text, true).unwrap(),
    ];
    let (query, dependencies) = prepare(
        "Contacts",
        "[FirstName] & ' ' & [LastName] & [FIRSTNAME]",
        &columns,
        ParseLimits::default(),
        &mut ResourceBudget::new(1024 * 1024, 100, 10000),
        &|| false,
    )
    .unwrap();
    assert_eq!(dependencies, ["FirstName", "LastName"]);
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Contacts".into(),
            columns,
            vec![vec![
                Value::Text("Ada".into()),
                Value::Text("Lovelace".into()),
            ]],
        )
        .unwrap(),
    ])
    .unwrap();
    let result = bind(&query, &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Text("Ada LovelaceAda".into())]]);
}

#[test]
fn same_row_predicates_preserve_comparison_and_nullable_boolean_tables() {
    let columns = vec![Column::new("Input".into(), ScalarType::Int32, true).unwrap()];
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            columns.clone(),
            vec![
                vec![Value::Int32(-1)],
                vec![Value::Int32(1)],
                vec![Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap();

    for (expression, expected) in [
        ("[Input]=1", [Some(false), Some(true), None]),
        ("[Input]<>1", [Some(true), Some(false), None]),
        ("[Input]<1", [Some(true), Some(false), None]),
        ("[Input]<=1", [Some(true), Some(true), None]),
        ("[Input]>1", [Some(false), Some(false), None]),
        ("[Input]>=1", [Some(false), Some(true), None]),
        ("Not ([Input]>0)", [Some(true), Some(false), None]),
        (
            "([Input]>0) And False",
            [Some(false), Some(false), Some(false)],
        ),
        ("([Input]>0) Or True", [Some(true), Some(true), Some(true)]),
        ("([Input]>0) Xor True", [Some(true), Some(false), None]),
        ("([Input]>0) Eqv False", [Some(true), Some(false), None]),
        ("([Input]>0) Imp True", [Some(true), Some(true), Some(true)]),
    ] {
        let (query, dependencies) = prepare(
            "Items",
            expression,
            &columns,
            ParseLimits::default(),
            &mut ResourceBudget::new(1024 * 1024, 100, 10000),
            &|| false,
        )
        .unwrap();
        assert_eq!(dependencies, ["Input"], "{expression}");

        let result = bind(&query, &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        let expected = expected.map(|value| vec![value.map_or(Value::Null, Value::Boolean)]);
        assert_eq!(result.rows(), expected, "{expression}");
    }
}

#[test]
fn admitted_logical_operators_retain_integer_masks_and_eager_failures() {
    let columns = vec![Column::new("Input".into(), ScalarType::Int32, false).unwrap()];
    let source = MemorySource::new(vec![
        MemoryTable::new("Items".into(), columns.clone(), vec![vec![Value::Int32(5)]]).unwrap(),
    ])
    .unwrap();

    for (expression, expected) in [
        ("Not [Input]", Value::Int32(-6)),
        ("[Input] And 3", Value::Int64(1)),
        ("[Input] Or 3", Value::Int64(7)),
        ("[Input] Xor 3", Value::Int64(6)),
        ("[Input] Eqv 3", Value::Int64(-7)),
        ("[Input] Imp 3", Value::Int64(-5)),
    ] {
        let (query, _) = prepare(
            "Items",
            expression,
            &columns,
            ParseLimits::default(),
            &mut ResourceBudget::new(1024 * 1024, 100, 10000),
            &|| false,
        )
        .unwrap();
        let result = bind(&query, &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();

        // Binary literals select Int64; unary NOT preserves the Int32 field width.
        assert_eq!(result.rows(), &[vec![expected]], "{expression}");
    }

    for expression in ["False And (1/0=0)", "True Or (1/0=0)", "NULL=(1/0)"] {
        let (query, _) = prepare(
            "Items",
            expression,
            &columns,
            ParseLimits::default(),
            &mut ResourceBudget::new(1024 * 1024, 100, 10000),
            &|| false,
        )
        .unwrap();
        let error = bind(&query, &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .err();

        assert_eq!(error, Some(EvalError::DivisionByZero.into()));
    }
}

#[test]
fn predicates_do_not_hide_unsupported_dependencies_in_inactive_operands() {
    let columns = vec![Column::new("Input".into(), ScalarType::Int32, true).unwrap()];

    for expression in [
        "False And (Now()=0)",
        "True Or ([Other].[Input]=0)",
        "False And ([Missing]=0)",
        "True Or ((SELECT 1)=1)",
    ] {
        assert!(
            prepare(
                "Items",
                expression,
                &columns,
                ParseLimits::default(),
                &mut ResourceBudget::new(1024 * 1024, 100, 10000),
                &|| false,
            )
            .is_err(),
            "{expression}"
        );
    }
}

#[test]
fn predicate_previews_keep_text_comparison_an_explicit_host_policy() {
    struct Ordinal;

    impl Collation for Ordinal {
        fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
            Ok(left.cmp(right))
        }
    }

    let columns = vec![Column::new("Label".into(), ScalarType::Text, false).unwrap()];
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            columns.clone(),
            vec![vec![Value::Text("A".into())], vec![Value::Text("a".into())]],
        )
        .unwrap(),
    ])
    .unwrap();
    let (query, _) = prepare(
        "Items",
        "[Label]='a'",
        &columns,
        ParseLimits::default(),
        &mut ResourceBudget::new(1024 * 1024, 100, 10000),
        &|| false,
    )
    .unwrap();
    let plan = bind(&query, &source).unwrap();

    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(EvalError::CollationRequired.into())
    );
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(
        result.rows(),
        [vec![Value::Boolean(false)], vec![Value::Boolean(true)]]
    );
}

#[test]
fn owned_calculation_source_preserves_literal_name_restrictions() {
    let mut budget = ResourceBudget::new(1 << 24, 100, 1 << 20);
    let (query, dependencies) = prepare(
        "Order Details",
        "1",
        &[],
        ParseLimits::default(),
        &mut budget,
        &|| false,
    )
    .unwrap();
    let expected = parse_query(
        "SELECT 1 AS [Recomputed] FROM [Order Details]",
        ParseLimits::default(),
    )
    .unwrap()
    .into_query();
    assert_eq!(query, expected);
    assert!(dependencies.is_empty());

    for table in ["", "a.b", "a!b", "a`b", "a[b", "a]b", "a\nb"] {
        assert!(
            prepare(
                table,
                "1",
                &[],
                ParseLimits::default(),
                &mut budget,
                &|| false,
            )
            .is_err(),
            "unexpected source admission: {table:?}"
        );
    }
}
