use super::*;

#[test]
fn hundred_thousand_generated_rows_need_no_parent_payload_or_buffered_callback() {
    let source = Generated::new(100_001);
    let syntax = parse_query("SELECT Count(*) AS Population FROM T", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut output = Vec::new();
    let options = ExecutionOptions {
        limits: QueryLimits {
            bytes: 8192,
            ..wide_options().limits
        },
        ..wide_options()
    };
    let summary = plan
        .execute_progressive(&source, &[], options, &mut |row| {
            assert!(source.eof.get());
            assert!(!source.active.get());
            output.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();

    assert_eq!(output, [vec![Value::Int64(100_001)]]);
    assert_eq!(summary.scanned_rows, 100_001);
    assert_eq!(summary.emitted_rows, 1);
    assert_eq!(source.visits.get(), 100_001);
    assert_eq!(*source.projections.borrow(), [Vec::<usize>::new()]);
    assert_eq!(source.buffered_calls.get(), 0);
}

#[test]
fn empty_filtered_and_aliased_count_values_match_buffered_global_groups() {
    for count in [0, 1, 7] {
        for sql in [
            "SELECT Count(*) AS Population,Population+CLngLng(1) AS NextValue,Count(*) AS Repeated FROM T",
            "SELECT Count(*) AS Population FROM T WHERE N Mod 2=0",
            "SELECT Count(*) AS Population FROM T WHERE False",
            "SELECT LenB(IIf(Count(*)=0,ChrB(65),ChrW(55296))) AS UnitBytes FROM T",
        ] {
            let mut source = Generated::new(count);
            source.buffered_allowed = true;
            let syntax = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(syntax.query(), &source).unwrap();
            let expected = plan.execute(&source, &[], Default::default()).unwrap();
            let mut output = Vec::new();
            let summary = plan
                .execute_progressive(&source, &[], Default::default(), &mut |row| {
                    assert!(source.eof.get());
                    assert!(!source.active.get());
                    output.push(row.to_vec());
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(output, expected.rows(), "{sql}");
            assert_eq!(summary.scanned_rows, count as u64);
            assert_eq!(summary.emitted_rows, 1);
            let expected_projection = if sql.contains("N Mod") {
                vec![0]
            } else {
                vec![]
            };
            assert_eq!(*source.projections.borrow(), [expected_projection], "{sql}");
        }
    }
}

#[test]
fn typed_parameter_composition_reuses_count_without_changing_its_carrier() {
    let source = Generated::new(3);
    let syntax = parse_query(
        "PARAMETERS P Long; SELECT Count(*) AS Population,Population+P AS Adjusted FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for declared in [false, true] {
        for parameter in [-4, 10] {
            let mut output = Vec::new();
            plan.execute_progressive(
                &source,
                &[("P", Value::Int32(parameter))],
                ExecutionOptions {
                    declared_integral: declared,
                    ..Default::default()
                },
                &mut |row| {
                    output.push(row.to_vec());
                    Ok::<_, ()>(())
                },
            )
            .unwrap();
            assert_eq!(
                output,
                [vec![
                    Value::Int64(3),
                    Value::Int64(3 + i64::from(parameter))
                ]]
            );
        }
    }
}

#[test]
fn count_and_row_union_arms_promote_in_written_order_after_the_count_eof() {
    let source = Generated::new(2);
    let syntax = parse_query(
        "SELECT Count(*) AS Amount FROM T UNION ALL SELECT CDec(N) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut output = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            if output.is_empty() {
                assert!(source.eof.get());
                assert!(!source.active.get());
            }
            output.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    let expected = [2, 1, 2].map(|value| vec![Value::Decimal(Decimal::new(value, 0).unwrap())]);
    assert_eq!(output, expected);
    assert_eq!(summary.scanned_rows, 4);
    assert_eq!(summary.emitted_rows, 3);
    assert_eq!(*source.projections.borrow(), [vec![], vec![0]]);
}

#[test]
fn separate_filtered_count_arms_do_not_share_their_precomputed_overlay() {
    let source = Generated::new(3);
    let syntax = parse_query(
        "SELECT Count(*) FROM T WHERE N<=1 UNION ALL SELECT Count(*) FROM T WHERE N<=2",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut output = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            assert!(source.eof.get());
            assert!(!source.active.get());
            output.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(output, [vec![Value::Int64(1)], vec![Value::Int64(2)]]);
    assert_eq!(summary.scanned_rows, 6);
    assert_eq!(*source.projections.borrow(), [vec![0], vec![0]]);
}
