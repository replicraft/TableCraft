use super::*;

#[test]
fn null_empty_text_false_error_and_raw_operands_use_existing_count_nullness() {
    let source = Generated::new(3);
    let sql = "SELECT Count(*),Count(NULL),Count(''),Count(False),Count(CVErr(7)),Count(ChrW(55296)),Count(ChrB(65)),Count(Nz(NULL)),Count(A) FROM T";
    let output = rows(sql, &source, Default::default());
    assert_eq!(
        output,
        [vec![
            Value::Int64(3),
            Value::Int64(0),
            Value::Int64(3),
            Value::Int64(3),
            Value::Int64(3),
            Value::Int64(3),
            Value::Int64(3),
            Value::Int64(3),
            Value::Int64(2),
        ]]
    );
    assert_eq!(*source.projections.borrow(), [vec![1]]);
    assert_eq!(source.buffered_calls.get(), 0);
}

#[test]
fn small_inputs_match_buffered_counts_and_keep_each_occurrence_identity() {
    for count in [0, 1, 7] {
        let mut source = Generated::new(count);
        source.buffered_allowed = true;
        for sql in [
            "SELECT Count(*),Count(N),Count(A),Count(IIf(N Mod 2=0,NULL,ChrW(55296))) FROM T",
            "SELECT Count(N) AS Population,Population+CLngLng(1) AS NextValue,Count(NULL) AS EmptyCount FROM T WHERE N>1",
            "SELECT Round(Count(N),Count(NULL)),Round(Count(NULL),Count(N)) FROM T",
        ] {
            let syntax = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(syntax.query(), &source).unwrap();
            let buffered = plan.execute(&source, &[], Default::default()).unwrap();
            assert_eq!(
                rows(sql, &source, Default::default()),
                buffered.rows(),
                "{sql}"
            );
        }
    }

    let source = Generated::new(3);
    let output = rows(
        "SELECT Round(Count(N),Count(NULL)),Round(Count(NULL),Count(N)) FROM T",
        &source,
        Default::default(),
    );
    assert_eq!(output, [vec![Value::Int64(3), Value::Int64(0)]]);
}

#[test]
fn argument_comparisons_run_once_per_occurrence_and_nulls_need_no_provider() {
    let mut source = Generated::new(3);
    source.policies[1] = Some(TextCollationId::from_host(1));
    let resolver = Resolver(Cell::new(0));
    let output = rows(
        "SELECT Count(A='a'),Count(A='a'),Count(*) FROM T",
        &source,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert_eq!(
        output,
        [vec![Value::Int64(2), Value::Int64(2), Value::Int64(3)]]
    );
    assert_eq!(resolver.0.get(), 4);

    let source = Generated::new(3);
    let unused = Resolver(Cell::new(0));
    let output = rows(
        "SELECT Count(A) FROM T",
        &source,
        ExecutionOptions {
            collation_resolver: Some(&unused),
            ..Default::default()
        },
    );
    assert_eq!(output, [vec![Value::Int64(2)]]);
    assert_eq!(unused.0.get(), 0);
}

#[test]
fn large_scalar_counts_use_only_argument_fields_without_a_parent_snapshot() {
    let source = Generated::new(100_001);
    let output = rows(
        "SELECT Count(N),Count(NULL) FROM T",
        &source,
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 32 * 1024 * 1024,
                rows: 300_000,
                steps: 10_000_000,
            },
            ..Default::default()
        },
    );
    assert_eq!(output, [vec![Value::Int64(100_001), Value::Int64(0)]]);
    assert_eq!(*source.projections.borrow(), [vec![0]]);
    assert_eq!(source.visits.get(), 100_001);
    assert_eq!(source.buffered_calls.get(), 0);
}

#[test]
fn count_metadata_parameters_and_union_promotions_keep_existing_profiles() {
    let source = Generated::new(3);
    let syntax = parse_query(
        "PARAMETERS P Long; SELECT Count(IIf(N>P,N,NULL)) AS Population FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [OutputType::Scalar {
            data_type: TypeDescriptor::new(ScalarType::Int64),
            nullable: false,
        }]
    );
    for declared in [false, true] {
        let mut output = Vec::new();
        plan.execute_progressive(
            &source,
            &[("P", Value::Int32(1))],
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
        assert_eq!(output, [vec![Value::Int64(2)]]);
    }

    let syntax = parse_query(
        "SELECT Count(A) AS Amount FROM T UNION ALL SELECT CDec(N) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut output = Vec::new();
    plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        output.push(row.to_vec());
        Ok::<_, ()>(())
    })
    .unwrap();
    let expected = [2, 1, 2, 3].map(|value| vec![Value::Decimal(Decimal::new(value, 0).unwrap())]);
    assert_eq!(output, expected);
}
