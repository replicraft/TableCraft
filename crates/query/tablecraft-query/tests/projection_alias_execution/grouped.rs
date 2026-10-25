use super::*;

#[test]
fn group_key_count_and_sum_aliases_consume_prior_group_outputs() {
    let source = Records::new();
    for declared in [false, true] {
        let options = ExecutionOptions {
            declared_integral: declared,
            ..Default::default()
        };
        let count_sql = "SELECT N AS Bucket,Count(*) AS Population,Population+CLngLng(1) AS NextCount FROM T GROUP BY N ORDER BY Bucket";
        let count_rows = run(count_sql, &source, options).unwrap();
        let expected_counts = [
            vec![Value::Int32(1), Value::Int64(2), Value::Int64(3)],
            vec![Value::Int32(2), Value::Int64(1), Value::Int64(2)],
        ];
        assert_eq!(count_rows, expected_counts);

        let sum_sql = "SELECT N AS Bucket,Sum(N) AS Amount,Amount+CLngLng(1) AS NextAmount FROM T GROUP BY N ORDER BY Bucket";
        let sum_rows = run(sum_sql, &source, options).unwrap();
        let expected_sums = [
            vec![Value::Int32(1), Value::Int64(2), Value::Int64(3)],
            vec![Value::Int32(2), Value::Int64(2), Value::Int64(3)],
        ];
        assert_eq!(sum_rows, expected_sums);
    }
}

#[test]
fn empty_global_groups_keep_count_values_and_unknown_widening_categories() {
    let mut source = Records::new();
    source.rows.clear();
    let count_sql = "SELECT Count(*) AS Population,Population+CLngLng(1) AS NextCount FROM T";
    let empty_counts = run(count_sql, &source, Default::default()).unwrap();
    assert_eq!(empty_counts, [vec![Value::Int64(0), Value::Int64(1)]]);

    let source = Records::new();
    let options = ExecutionOptions {
        declared_integral: true,
        ..Default::default()
    };
    let unknown_sql = "SELECT First(CInt(2)) AS Chosen,Chosen+CInt(32767) AS Widening FROM T";
    let unknown = run(unknown_sql, &source, options);
    assert_eq!(unknown.err(), Some(QueryError::ArithmeticDeclaration));

    let variant_sql = "SELECT First(CInt(2)) AS Chosen,CVar(Chosen)+CInt(32767) AS Widening FROM T";
    let variant = run(variant_sql, &source, options).unwrap();
    assert_eq!(variant, [vec![Value::Int16(2), Value::Int32(32769)]]);
}

#[test]
fn grouped_aliases_keep_required_and_completed_string_policies() {
    let source = Records::new();
    for expression in ["A", "A&IIf(True,0,B)"] {
        let expected = if expression == "A" { "A" } else { "A0" };
        let sql = format!(
            "SELECT {expression} AS Label,Count(*) AS Population,Label='{expected}' AS Matches FROM T GROUP BY {expression} ORDER BY Population DESC"
        );
        let rows = run(
            &sql,
            &source,
            ExecutionOptions {
                collation_resolver: Some(&Resolver),
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            rows.iter().map(|row| row[2].clone()).collect::<Vec<_>>(),
            [Value::Boolean(true), Value::Boolean(false)]
        );
        assert_eq!(
            rows.iter().map(|row| row[1].clone()).collect::<Vec<_>>(),
            [Value::Int64(2), Value::Int64(1)]
        );
    }
    let conflicting_sql =
        "SELECT A&B AS Label,Count(*) AS Population,Label=Label AS Matches FROM T GROUP BY A&B";
    let conflicting = run(
        conflicting_sql,
        &source,
        ExecutionOptions {
            collation_resolver: Some(&Resolver),
            ..Default::default()
        },
    );
    assert_eq!(conflicting.err(), Some(QueryError::CollationConflict));
}

#[test]
fn raw_group_key_aliases_are_observed_without_publishing_the_inner_key() {
    let source = Records::new();
    let sql = "SELECT UnitBytes,Population FROM (SELECT A&ChrW(55296) AS RawKey,Count(*) AS Population,LenB(RawKey) AS UnitBytes FROM T GROUP BY A&ChrW(55296)) Q ORDER BY Population DESC";
    let options = ExecutionOptions {
        collation_resolver: Some(&Resolver),
        ..Default::default()
    };
    let rows = run(sql, &source, options).unwrap();
    let expected = [
        vec![Value::Int32(4), Value::Int64(2)],
        vec![Value::Int32(4), Value::Int64(1)],
    ];
    assert_eq!(rows, expected);
}

#[test]
fn alias_references_cannot_launder_invalid_group_fields_or_enter_aggregate_operands() {
    let source = Records::new();
    for sql in [
        "SELECT A AS Ungrouped,Count(*) AS Population,Ungrouped AS CopyValue FROM T GROUP BY N",
        "SELECT N AS Bucket,Sum(Bucket) AS Amount FROM T GROUP BY N",
        "SELECT N AS Bucket,Count(*) FROM T GROUP BY Bucket",
        "SELECT Count(*) AS Population FROM T HAVING Population>0",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        assert!(prepare(syntax.query(), &source).is_err(), "{sql}");
        assert_eq!(source.scans.get(), 0, "{sql}");
    }
}

#[test]
fn every_observed_group_alias_checkpoint_can_cancel() {
    let source = Records::new();
    let syntax = parse_query(
        "SELECT N AS Bucket,Count(*) AS Population,Population+CLngLng(1) AS NextCount FROM T GROUP BY N ORDER BY Bucket",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&observe),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
}
