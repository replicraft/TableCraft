use std::{
    cmp::Ordering,
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    comparison::Collation,
    functions::error_messages::{ErrorMessageCode, ErrorMessages},
};
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(1)], vec![Value::Int32(2)]],
        )
        .unwrap(),
    ])
    .unwrap()
}

struct Messages(&'static str);
impl ErrorMessages for Messages {
    fn message(&self, _: ErrorMessageCode) -> Result<&str, EvalError> {
        Ok(self.0)
    }
}

#[test]
fn message_context_is_execution_scoped_in_grouping_domains_and_nested_queries() {
    let source = source();
    for sql in [
        "SELECT Error(11),(SELECT Error(11)),Eval('Error(11)'),TypeName(Error(11))",
        "SELECT Error(ID),Count(*) FROM Items GROUP BY Error(ID)",
        "SELECT Error(11),Count(*) FROM Items GROUP BY Eval('Error(11)')",
        "SELECT DFirst('Error(ID)','Items'),DLast('Error(ID)','Items'),DLookup('Error(ID)','Items')",
        "SELECT Q.Message FROM (SELECT Error(11) AS Message) AS Q",
        "SELECT Error(11) FROM Items WHERE Error(ID)='message' ORDER BY Error(ID)",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        for name in ["message", "é中🦀"] {
            let messages = Messages(name);
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        error_messages: Some(&messages),
                        collation: Some(&Ordinal),
                        ..Default::default()
                    },
                )
                .unwrap();
            if let Some(row) = result.rows().first() {
                assert_eq!(row[0], Value::Text(name.into()), "{sql}");
            } else {
                assert_eq!(name, "é中🦀");
            }
        }
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(EvalError::ErrorCatalogRequired.into()),
            "{sql}"
        );
    }
}

#[test]
fn invalid_arity_and_qualified_calls_are_rejected_at_binding() {
    let source = source();
    for sql in [
        "SELECT Error()",
        "SELECT Error(1,1/0)",
        "SELECT VBA.Error(11)",
        "SELECT Error(*)",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert!(prepare(query.query(), &source).is_err(), "{sql}");
    }
}

struct CountedMessages(AtomicUsize);
impl ErrorMessages for CountedMessages {
    fn message(&self, _: ErrorMessageCode) -> Result<&str, EvalError> {
        self.0.fetch_add(1, AtomicOrdering::Relaxed);
        Ok("selected")
    }
}

#[test]
fn numbered_messages_refuse_error_states_before_catalog_in_derived_and_aggregate_paths() {
    let source = source();
    let messages = CountedMessages(AtomicUsize::new(0));
    for function in ["Error", "Error$"] {
        for sql in [
            format!("SELECT {function}(CVErr(11))"),
            format!("SELECT {function}(Q.E) FROM (SELECT CVErr(11) AS E) AS Q"),
            format!("SELECT {function}((SELECT CVErr(11)))"),
            format!("SELECT {function}(First(CVErr(ID))) FROM Items"),
            format!("SELECT {function}(DFirst('CVErr(ID)','Items'))"),
        ] {
            let parsed = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            for options in [
                ExecutionOptions::default(),
                ExecutionOptions {
                    error_messages: Some(&messages),
                    ..Default::default()
                },
            ] {
                assert_eq!(
                    plan.execute(&source, &[], options).err(),
                    Some(EvalError::TypeMismatch.into()),
                    "{sql}"
                );
            }
        }
    }
    assert_eq!(messages.0.load(AtomicOrdering::Relaxed), 0);
    let parsed = parse_query(
        "SELECT Error(CLng(CVErr(11))),Error$(CLng(CVErr(11)))",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    error_messages: Some(&messages),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        &[vec![
            Value::Text("selected".into()),
            Value::Text("selected".into())
        ]]
    );
    assert_eq!(messages.0.load(AtomicOrdering::Relaxed), 2);
}

#[test]
fn numbered_message_state_failure_does_not_poison_plan_reuse_or_call_catalog() {
    let source = source();
    let messages = CountedMessages(AtomicUsize::new(0));
    for function in ["Error", "Error$"] {
        let parsed = parse_query(
            &format!("PARAMETERS P LONG; SELECT {function}(IIf(P=0,0,IIf(P=1,CVErr(11),11)))"),
            Default::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        for (parameter, expected) in [
            (1, Err(EvalError::TypeMismatch.into())),
            (0, Ok(vec![vec![Value::Text(String::new())]])),
            (2, Ok(vec![vec![Value::Text("selected".into())]])),
        ] {
            let result = plan
                .execute(
                    &source,
                    &[("P", Value::Int32(parameter))],
                    ExecutionOptions {
                        error_messages: Some(&messages),
                        ..Default::default()
                    },
                )
                .map(|result| result.rows().to_vec());
            assert_eq!(result, expected);
        }
    }
    assert_eq!(messages.0.load(AtomicOrdering::Relaxed), 2);
}

#[test]
fn numbered_message_recognition_does_not_suppress_operational_operand_failures() {
    let source = source();
    let messages = CountedMessages(AtomicUsize::new(0));
    for function in ["Error", "Error$"] {
        for sql in [
            format!("SELECT {function}(IIf(True,CVErr(11),1/0))"),
            format!(
                "SELECT {function}(Q.E) FROM (SELECT IIf(True,CVErr(ID),1/(ID-1)) AS E FROM Items) AS Q"
            ),
        ] {
            let parsed = parse_query(&sql, Default::default()).unwrap();
            assert_eq!(
                prepare(parsed.query(), &source)
                    .unwrap()
                    .execute(
                        &source,
                        &[],
                        ExecutionOptions {
                            error_messages: Some(&messages),
                            ..Default::default()
                        }
                    )
                    .err(),
                Some(EvalError::DivisionByZero.into()),
                "{sql}"
            );
        }
    }
    assert_eq!(messages.0.load(AtomicOrdering::Relaxed), 0);
}
