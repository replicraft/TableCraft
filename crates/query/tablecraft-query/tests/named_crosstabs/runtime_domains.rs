use super::*;

#[test]
fn discovered_domain_results_keep_variant_returns_in_declared_arithmetic() {
    let source = Views::new(&[("Piv", DYNAMIC)]);
    let sql =
        "SELECT DLookup('CByte(255)','Piv','Region=1')+CByte(1),DMin('CInt(32767)','Piv')+CInt(1)";
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = run(
        &source,
        sql,
        &[],
        ExecutionOptions {
            declared_integral: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [row(vec![Value::Int16(256), Value::Int32(32768)])]
    );
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn runtime_only_domains_discover_once_and_legacy_execution_stays_explicit() {
    let source = Views::new(&[("Piv", DYNAMIC)]);
    let sql =
        "SELECT DSum('[2]','Piv'),DCount('*','Piv','Region>1'),DLookup('[3]','Piv','Region=2')";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute_expression(&source, &[], Default::default())
            .err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(source.scans.get(), 0);
    let result = run(&source, sql, &[], Default::default()).unwrap();
    assert_eq!(
        result.rows(),
        [row(vec![
            Value::Int64(17),
            Value::Int64(2),
            Value::Int64(5)
        ])]
    );
    assert_eq!(source.scans.get(), 1);
    assert!(run(&source, sql, &[], Default::default()).is_ok());
    assert_eq!(source.scans.get(), 2);

    let source = Views::new(&[(
        "Context",
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN (CLng('2'))",
    )]);
    let result = run(
        &source,
        "SELECT DLookup('[2]','Context','Region=1')",
        &[],
        ExecutionOptions {
            number_format: Some(NumberFormat::EnUs),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.rows(), [row(vec![Value::Int64(17)])]);
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn runtime_discovery_extends_a_live_cache_and_keeps_nested_cycles_bounded() {
    let source = Views::new(&[
        ("Piv", DYNAMIC),
        (
            "Other",
            "TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter",
        ),
        (
            "Nested",
            "TRANSFORM First(DCount('*','Other')) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter",
        ),
    ]);
    let result = run(
        &source,
        r#"SELECT Region,[2],DLookup('[2]','Nested','Region=1') AS N,DCount('*','Other','DCount("*","Piv")=3') AS C FROM Piv ORDER BY Region"#,
        &[],
        Default::default(),
    ).unwrap();
    assert_eq!(
        result.rows(),
        [
            row(vec![
                Value::Int32(1),
                Value::Int64(17),
                Value::Int64(3),
                Value::Int64(3)
            ]),
            row(vec![
                Value::Int32(2),
                Value::Null,
                Value::Int64(3),
                Value::Int64(3)
            ]),
            row(vec![
                Value::Int32(3),
                Value::Null,
                Value::Int64(3),
                Value::Int64(3)
            ]),
        ]
    );
    assert_eq!(source.scans.get(), 3);
    let source = Views::new(&[(
        "Piv",
        "TRANSFORM First(DCount('*','Piv')) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter",
    )]);
    assert_eq!(
        run(&source, "SELECT DCount('*','Piv')", &[], Default::default()).err(),
        Some(QueryError::DependencyCycle)
    );
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn runtime_domains_reject_transitive_parameters_before_discovery_reads() {
    for definitions in [
        vec![(
            "Piv",
            "PARAMETERS P LONG; TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN (P)",
        )],
        vec![
            ("Piv", "PARAMETERS P LONG; SELECT * FROM Child"),
            ("Child", DYNAMIC),
        ],
        vec![
            ("Piv", "SELECT * FROM Child"),
            (
                "Child",
                "PARAMETERS P LONG; TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter",
            ),
        ],
    ] {
        let source = Views::new(&definitions);
        assert_eq!(
            run(&source, "SELECT DCount('*','Piv')", &[], Default::default()).err(),
            Some(QueryError::Parameter)
        );
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn runtime_discovery_uses_one_budget_and_observes_every_cancellation_checkpoint() {
    let source = Views::new(&[("Piv", DYNAMIC)]);
    let sql = "SELECT DSum('[2]','Piv'),DCount('*','Piv')";
    let minimum = (1..=256)
        .find(|rows| {
            run(
                &source,
                "SELECT DSum('[2]','Piv')",
                &[],
                ExecutionOptions {
                    limits: QueryLimits {
                        rows: *rows,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .is_ok()
        })
        .unwrap();
    assert!(matches!(run(&source, sql, &[], ExecutionOptions {
        limits: QueryLimits { rows: minimum, ..Default::default() }, ..Default::default()
    }), Err(QueryError::Limit(error)) if error.resource == tablecraft_core::limits::Resource::Rows));
    let calls = Cell::new(0);
    let check = |stop| {
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        run(
            &source,
            sql,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    assert!(check(usize::MAX).is_ok());
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert!(
            matches!(
                check(stop),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
}

#[test]
fn runtime_discovery_never_retains_late_failures_or_changed_definitions_between_runs() {
    let source = Changing {
        inner: Views::new(&[("Piv", DYNAMIC)]),
        alternate: parse_query(DYNAMIC, Default::default())
            .unwrap()
            .into_query(),
        fail: Cell::new(true),
    };
    assert_eq!(run_changing(&source).err(), Some(QueryError::Source));
    source.fail.set(false);
    assert!(run_changing(&source).is_ok());
    let source = Changing {
        inner: Views::new(&[("Piv", DYNAMIC)]),
        alternate: parse_query("SELECT Region FROM Sales", Default::default())
            .unwrap()
            .into_query(),
        fail: Cell::new(false),
    };
    assert_eq!(run_changing(&source).err(), Some(QueryError::SchemaChanged));
}

fn run_changing(source: &Changing) -> Result<ExpressionQueryResult, QueryError> {
    let syntax = parse_query("SELECT DCount('*','Piv')", Default::default()).unwrap();
    execute_with_schema_discovery(syntax.query(), source, &[], Default::default())
}
