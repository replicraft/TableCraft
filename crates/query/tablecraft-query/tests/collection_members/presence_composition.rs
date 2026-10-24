use super::criteria::run_text;
use super::*;
use tablecraft_expressions::{EvalError, functions::math::random::RndState};
use tablecraft_query::QueryLimits;

#[test]
fn scalar_composition_evaluates_presence_as_boolean_without_child_expansion() {
    let source = Attachments::new();
    for (predicate, identities) in [
        ("(Files IS NULL)=True", vec![2]),
        ("(Files IS NULL)=False", vec![1, 3]),
        ("CBool(Files IS NOT NULL)", vec![1, 3]),
        ("CVar(Files IS NULL)=True", vec![2]),
        ("Abs(Files IS NULL)=1", vec![2]),
        ("Len(IIf(Files IS NULL,'x','yy'))=1", vec![2]),
        ("Eval(IIf(Files IS NULL,'True','False'))", vec![2]),
        ("Nz(NULL,Files IS NULL)", vec![2]),
        ("True IN (Files IS NULL,False)", vec![2]),
        ("VarType(Files IS NULL)=11", vec![1, 2, 3]),
        ("IsArray(Array(Files IS NULL))", vec![1, 2, 3]),
        ("SLN(IIf(Files IS NULL,3,5),1,2)=1", vec![2]),
        (
            "DateSerial(1900,1,IIf(Files IS NULL,1,2))=#1900-01-01#",
            vec![2],
        ),
        (
            "DateDiff('d',#1900-01-01#,#1900-01-02#,1,IIf(Files IS NULL,1,2))=1",
            vec![1, 2, 3],
        ),
        (
            "DatePart('d',#1900-01-01#,1,IIf(Files IS NULL,1,2))=1",
            vec![1, 2, 3],
        ),
        ("Weekday(#1900-01-01#,IIf(Files IS NULL,1,2))=2", vec![2]),
        ("(Files IS NULL) IN (SELECT True)", vec![2]),
        ("(Files IS NULL)=ANY(SELECT True)", vec![2]),
        ("IIf(Files IS NULL,True,False)", vec![2]),
        ("Switch(Files IS NULL,True,True,False)", vec![2]),
        ("Choose(IIf(Files IS NULL,1,2),True,False)", vec![2]),
        ("(Files IS NULL) IN (True)", vec![2]),
        ("(Files IS NULL) BETWEEN False AND False", vec![1, 3]),
        ("IsNull(Files IS NULL)=False", vec![1, 2, 3]),
        (
            "IIf(Files IS NULL,Other IS NOT NULL,Other IS NULL)",
            vec![2, 3],
        ),
    ] {
        let sql = format!("SELECT Id FROM Items WHERE {predicate} ORDER BY Id");
        let query = parse_query(&sql, ParseLimits::default()).unwrap();
        let scans = source.scans.get();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(source.scans.get(), scans);
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        let expected: Vec<_> = identities
            .into_iter()
            .map(|id| vec![Value::Int32(id)])
            .collect();
        assert_eq!(result.rows(), expected, "{sql}");
    }
}

#[test]
fn composed_presence_keeps_displayed_member_filters_and_nested_read_contexts_separate() {
    let source = Attachments::new();
    let sql =
        "SELECT Id,Files.FileName FROM Items WHERE IIf(Files IS NULL,False,Files.FileName='a')";
    assert_eq!(
        run_text(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1), text("a")]]
    );
    assert_eq!(
        run(
            "SELECT DCount('*','Items','IIf(Files IS NULL,True,False)')",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int64(1)]]
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE (Files IS NULL)=(SELECT True)",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(2)]]
    );

    let nested = "SELECT Id FROM Items WHERE IIf(Files IS NULL,EXISTS(SELECT 1 FROM Items AS I WHERE I.Other IS NULL),False)";
    assert_eq!(
        run(nested, &source).unwrap().rows(),
        [vec![Value::Int32(2)]]
    );

    for sql in [
        "SELECT Id FROM Items WHERE IIf(Files IS NULL,True,Files.FileName='a')",
        "SELECT Id FROM Items WHERE Files IS NULL OR (SELECT Files IS NULL FROM Items)",
        "SELECT Id FROM Items WHERE (SELECT Files IS NULL FROM Items)",
        "SELECT Id FROM Items AS T WHERE (SELECT 1 WHERE T.Files IS NULL)=1",
        "SELECT Id FROM Items WHERE True ORDER BY Files IS NULL",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let scans = source.scans.get();
        assert!(prepare(query.query(), &source).is_err(), "{sql}");
        assert_eq!(source.scans.get(), scans);
    }
}

#[test]
fn scalar_presence_composition_preserves_eager_failures_and_once_only_volatile_calls() {
    let source = Attachments::new();
    for predicate in [
        "IIf(Files IS NULL,True,1/0)",
        "IIf(False,1/0,Files IS NULL)",
        "(Files IS NULL)=(1/0)",
    ] {
        let sql = format!("SELECT Id FROM Items WHERE {predicate}");
        assert_eq!(
            run(&sql, &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }

    let state = RndState::default();
    let expected = RndState::default();
    for _ in 0..3 {
        expected.apply(None).unwrap();
    }
    let query = parse_query(
        "SELECT Id FROM Items WHERE IIf(Rnd()>=0,Files IS NULL,False)",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(query.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int32(2)]]);
    assert_eq!(state.apply(None), expected.apply(None));
}

#[test]
fn presence_validation_occurs_in_written_argument_order_instead_of_a_prefetch_pass() {
    let mut source = Attachments::new();
    source.lists[0][0] = vec![Value::Int32(42)];
    let division = QueryError::Evaluation(EvalError::DivisionByZero);
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE IIf(1/0,Files IS NULL,False)",
            &source
        )
        .err(),
        Some(division.clone())
    );
    let result = run(
        "SELECT Id FROM Items WHERE IIf(Files IS NULL,1/0,False)",
        &source,
    );
    assert!(result.is_err());
    assert_ne!(result.err(), Some(division));
}

#[test]
fn composed_presence_obeys_shared_limits_and_all_observed_cancellation_checkpoints() {
    let source = Attachments::new();
    for sql in [
        "SELECT Id FROM Items WHERE IIf(Files IS NULL,True,False)",
        "SELECT DCount('*','Items','CBool(Files IS NULL)')",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        let checks = Cell::new(0);
        let invoke = |stop| {
            checks.set(0);
            let cancelled = || {
                checks.set(checks.get() + 1);
                checks.get() == stop
            };
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                },
            )
        };
        assert!(invoke(usize::MAX).is_ok());
        let total = checks.get();
        for stop in 1..=total {
            assert!(
                matches!(
                    invoke(stop).err(),
                    Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
                ),
                "{sql}: checkpoint {stop}"
            );
        }
        for limits in [
            QueryLimits {
                rows: 1,
                ..Default::default()
            },
            QueryLimits {
                bytes: 1,
                ..Default::default()
            },
            QueryLimits {
                steps: 1,
                ..Default::default()
            },
        ] {
            assert!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        limits,
                        ..Default::default()
                    }
                )
                .is_err(),
                "{sql}"
            );
        }
    }
}
