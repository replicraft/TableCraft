//! Literal selection refines carriers and constraints without skipping evaluation.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_expressions::functions::dates::clock::{Clock, ClockReading};

#[test]
fn literal_controls_keep_exact_selected_constraints_and_nullable_outcomes() {
    for (expression, expected) in [
        (
            "IIf(True,A,B)",
            descriptor(TypeDescriptor::decimal(3, 1).unwrap(), false),
        ),
        (
            "IIf(False,A,B)",
            descriptor(TypeDescriptor::decimal(5, 3).unwrap(), false),
        ),
        (
            "IIf(Null,A,[Short])",
            descriptor(TypeDescriptor::text(2).unwrap(), true),
        ),
        (
            "IIf(1.5,A,B)",
            descriptor(TypeDescriptor::decimal(3, 1).unwrap(), false),
        ),
        (
            "IIf(0,A,B)",
            descriptor(TypeDescriptor::decimal(5, 3).unwrap(), false),
        ),
        (
            "Choose(1,A,[Long])",
            descriptor(TypeDescriptor::decimal(3, 1).unwrap(), false),
        ),
        (
            "Choose(2,A,[Short])",
            descriptor(TypeDescriptor::text(2).unwrap(), true),
        ),
        (
            "Choose(1.49999999,A,B,[Long])",
            descriptor(TypeDescriptor::decimal(5, 3).unwrap(), false),
        ),
        (
            "Choose(2.50000001,A,B,[Long])",
            descriptor(TypeDescriptor::decimal(5, 3).unwrap(), false),
        ),
        ("Choose(0,A,[Long])", OutputType::Null),
        ("Choose(-1,A,[Long])", OutputType::Null),
        ("Choose(1e30,A,[Long])", OutputType::Null),
        ("Choose(Null,A,[Long])", OutputType::Null),
        (
            "Switch(False,A,True,B)",
            descriptor(TypeDescriptor::decimal(5, 3).unwrap(), false),
        ),
        (
            "Switch(True,[Short],True,[Long])",
            descriptor(TypeDescriptor::text(2).unwrap(), true),
        ),
        ("Switch(True,Null,True,A)", OutputType::Null),
        ("Switch(False,A,Null,B)", OutputType::Null),
        (
            "Switch(Flag,A,True,B)",
            descriptor(TypeDescriptor::new(ScalarType::Decimal), false),
        ),
        (
            "Switch(Flag,A,False,[Long])",
            descriptor(TypeDescriptor::decimal(3, 1).unwrap(), true),
        ),
        ("Choose(Flag,A,[Long])", OutputType::Dynamic),
        ("IIf(Flag,A,[Long])", OutputType::Dynamic),
        ("IIf('1',A,[Long])", OutputType::Dynamic),
    ] {
        let source = source();
        let parsed =
            parse_query(&format!("SELECT {expression} FROM T"), Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0, "{expression}");
        assert_eq!(plan.output_types().unwrap(), [expected], "{expression}");
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.output_types(), [expected]);
        for row in result.rows() {
            match expected {
                OutputType::Null => assert_eq!(row, &[Value::Null]),
                OutputType::Scalar {
                    data_type,
                    nullable,
                } => {
                    assert!(nullable || row[0] != Value::Null, "{expression}");
                    assert!(data_type.validate_value(&row[0]).is_ok(), "{expression}");
                }
                OutputType::Dynamic => {}
                OutputType::Multivalue { .. } => {
                    panic!("scalar selector unexpectedly has collection metadata")
                }
            }
        }
    }
}

#[test]
fn precise_descriptors_do_not_promise_success_or_hide_eager_errors() {
    for expression in [
        "IIf(True,A,1/0)",
        "Choose(1,A,1/0)",
        "Switch(True,A,True,1/0)",
        "Switch(True,A,1/0,B)",
    ] {
        let source = source();
        let parsed =
            parse_query(&format!("SELECT {expression} FROM T"), Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [descriptor(TypeDescriptor::decimal(3, 1).unwrap(), false)]
        );
        assert!(
            matches!(
                plan.execute(&source, &[], Default::default()),
                Err(QueryError::Evaluation(EvalError::DivisionByZero))
            ),
            "{expression}"
        );
    }

    let source = source();
    let parsed = parse_query("SELECT Choose(0,A,1/0) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(plan.output_types().unwrap(), [OutputType::Null]);
    assert!(matches!(
        plan.execute(&source, &[], Default::default()),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));

    let parsed = parse_query("SELECT Choose(1e100,A,[Long]) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
    assert!(matches!(
        plan.execute(&source, &[], Default::default()),
        Err(QueryError::Evaluation(EvalError::Overflow))
    ));
}

#[test]
fn selected_descriptors_compose_through_derived_reads_groups_and_empty_inputs() {
    for sql in [
        "SELECT Q.V FROM (SELECT IIf(True,A,[Long]) AS V FROM T) AS Q",
        "SELECT IIf(True,A,B) FROM T GROUP BY A,B",
        "SELECT Q.V FROM (SELECT Switch(False,[Long],True,A) AS V FROM T) AS Q WHERE False",
    ] {
        let source = source();
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.output_types().unwrap(),
            [descriptor(TypeDescriptor::decimal(3, 1).unwrap(), false)]
        );
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        for row in result.rows() {
            assert_eq!(row, &[Value::Decimal(Decimal::new(125, 1).unwrap())]);
        }
    }
}

#[test]
fn refined_metadata_keeps_raw_strings_and_variant_declarations() {
    for selector in [
        "IIf(True,ChrW(55296),1)",
        "Choose(1,ChrW(55296),1)",
        "Switch(True,ChrW(55296),True,1)",
    ] {
        let source = MemorySource::default();
        let parsed = parse_query(&format!("SELECT {selector}"), Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.output_types().unwrap(),
            [descriptor(TypeDescriptor::new(ScalarType::Text), true)]
        );
        assert!(plan.execute(&source, &[], Default::default()).is_err());

        let parsed = parse_query(&format!("SELECT LenB({selector})"), Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int32(2)]]);
    }

    for selector in [
        "IIf(True,CInt(32767),CDbl(0))",
        "Choose(1,CInt(32767),CDbl(0))",
        "Switch(True,CInt(32767),True,CDbl(0))",
    ] {
        let source = MemorySource::default();
        let parsed =
            parse_query(&format!("SELECT {selector}+CInt(1)"), Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int32(32768)]]);
    }
}

#[test]
fn known_choices_never_activate_providers_in_preparation_or_skip_them_at_runtime() {
    struct FailingClock(AtomicUsize);
    impl Clock for FailingClock {
        fn read(&self) -> Result<ClockReading, EvalError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(EvalError::ClockUnavailable)
        }
    }

    let source = source();
    let clock = FailingClock(AtomicUsize::new(0));
    for expression in [
        "IIf(True,A,Now())",
        "Choose(1,A,Now())",
        "Switch(True,A,True,Now())",
    ] {
        let parsed =
            parse_query(&format!("SELECT {expression} FROM T"), Default::default()).unwrap();
        let scans = source.scans.get();
        let calls = clock.0.load(Ordering::SeqCst);
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), scans);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert_eq!(
            plan.output_types().unwrap(),
            [descriptor(TypeDescriptor::decimal(3, 1).unwrap(), false)]
        );
        let calls = clock.0.load(Ordering::SeqCst);
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    clock: Some(&clock),
                    ..Default::default()
                },
            ),
            Err(QueryError::Evaluation(EvalError::ClockUnavailable))
        ));
        assert_eq!(clock.0.load(Ordering::SeqCst), calls + 1);
    }

    let parsed = parse_query("SELECT IIf(True,A,UnknownField) FROM T", Default::default()).unwrap();
    assert!(matches!(
        prepare(parsed.query(), &source),
        Err(QueryError::UnknownName)
    ));
}
