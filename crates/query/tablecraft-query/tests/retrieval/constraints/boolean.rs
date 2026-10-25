use super::*;

impl Restricted {
    fn boolean(ignore: bool) -> Self {
        Self {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", ScalarType::Boolean), ("Y", ScalarType::Int32)],
                vec![
                    vec![Value::Boolean(false), Value::Int32(1)],
                    vec![Value::Boolean(true), Value::Int32(2)],
                    vec![Value::Null, Value::Int32(3)],
                    vec![Value::Boolean(false), Value::Int32(4)],
                ],
            )])
            .unwrap(),
            seen: Cell::new(None),
            scans: Cell::new(0),
            ignore,
        }
    }
}

#[test]
fn exact_boolean_literal_hints_preserve_filters_nulls_reverse_operands_and_table_order() {
    for (filter, value) in [
        ("X=True", true),
        ("False=X", false),
        ("X=False And X=False", false),
    ] {
        let source = Restricted::boolean(false);
        let ignoring = Restricted::boolean(true);
        let sql = format!("SELECT Y FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            run(&sql, &ignoring).unwrap().rows()
        );
        assert_eq!(
            source.seen.get(),
            Some(ScanConstraint::Boolean { column: 0, value })
        );
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            if value {
                vec![vec![Value::Int32(2)]]
            } else {
                vec![vec![Value::Int32(1)], vec![Value::Int32(4)]]
            }
        );
    }
}

#[test]
fn boolean_coercions_ranges_null_tests_and_conflicting_or_partial_predicates_decline_hints() {
    for filter in [
        "X=-1",
        "X='True'",
        "X=CBool(1)",
        "X=CVar(True)",
        "X>False",
        "X<>True",
        "X IS NULL",
        "X IS NOT NULL",
        "X=True Or X=False",
        "X=True And X=False",
        "X=True And Y=2",
    ] {
        let source = Restricted::boolean(false);
        let ignoring = Restricted::boolean(true);
        let sql = format!("SELECT Y FROM T WHERE {filter}");
        let actual = run(&sql, &source).map(|result| result.rows().to_vec());
        let expected = run(&sql, &ignoring).map(|result| result.rows().to_vec());
        assert_eq!(actual, expected, "{sql}");
        assert_eq!(source.seen.get(), None, "{sql}");
    }
}

#[test]
fn boolean_hint_extraction_never_hides_eager_errors_or_promotes_parameters_to_literals() {
    for filter in [
        "X=False And 1/0=0",
        "X=True And CBool('bad')",
        "X=True And X=CBool(1/0)",
    ] {
        let source = Restricted::boolean(false);
        let sql = format!("SELECT Y FROM T WHERE {filter}");
        assert!(run(&sql, &source).is_err());
        assert_eq!(source.seen.get(), None);
    }
    let source = Restricted::boolean(false);
    let query = tablecraft_sql::parse_query(
        "PARAMETERS P BIT; SELECT Y FROM T WHERE X=P",
        Default::default(),
    )
    .unwrap();
    prepare(query.query(), &source)
        .unwrap()
        .execute(&source, &[("P", Value::Boolean(true))], Default::default())
        .unwrap();
    assert_eq!(source.seen.get(), None);
}
