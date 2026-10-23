use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::text::NumberFormat, error_messages::ErrorMessages,
        session::AccessSessionContext,
    },
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

struct Messages(AtomicUsize);
impl ErrorMessages for Messages {
    fn message(&self, _: u16) -> Result<&str, EvalError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok("-1")
    }
}

#[test]
fn contextual_return_metadata_does_not_activate_session_or_catalog() {
    let source = MemorySource::default();
    for (expression, variant) in [
        ("CurrentUser()", false),
        ("Error(1)", true),
        (
            "GUIDFromString('00112233-4455-6677-8899-AABBCCDDEEFF')",
            true,
        ),
        (
            "StringFromGUID(GUIDFromString('00112233-4455-6677-8899-AABBCCDDEEFF'))",
            true,
        ),
    ] {
        let sql = format!("SELECT CInt(-32768)\\(CInt(0)=ALL(SELECT {expression} WHERE False))");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        );
        if variant {
            assert_eq!(
                result.unwrap().rows(),
                &[vec![Value::Int32(32768)]],
                "{sql}"
            );
        } else {
            assert_eq!(result.err(), Some(EvalError::Overflow.into()), "{sql}");
        }
    }
}

#[test]
fn runtime_context_returns_keep_categories_and_provider_call_counts() {
    let source = MemorySource::default();
    let session = AccessSessionContext::new("-1").unwrap();
    let messages = Messages(AtomicUsize::new(0));
    for sql in [
        "SELECT CInt(-32768)\\Error(1)",
        "SELECT CInt(-32768)\\Q.Message FROM (SELECT Error(1) AS Message) AS Q",
        "SELECT CInt(-32768)\\(SELECT Error(1))",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let before = messages.0.load(Ordering::SeqCst);
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    error_messages: Some(&messages),
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[vec![Value::Int32(32768)]]
        );
        assert_eq!(messages.0.load(Ordering::SeqCst), before + 1);
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                }
            )
            .err(),
            Some(EvalError::ErrorCatalogRequired.into())
        );
    }
    let parsed = parse_query("SELECT CInt(-32768)\\CurrentUser()", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                access_session: Some(session),
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            }
        )
        .err(),
        Some(EvalError::Overflow.into())
    );
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            }
        )
        .err(),
        Some(EvalError::AccessSessionRequired.into())
    );
    let parsed = parse_query(
        "SELECT CVar(1)<StringFromGUID(GUIDFromString('00112233-4455-6677-8899-AABBCCDDEEFF'))",
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
                    declared_integral: true,
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        &[vec![Value::Boolean(true)]]
    );
    let parsed = parse_query("SELECT Error(1),1/0", Default::default()).unwrap();
    let result: Result<_, QueryError> = prepare(parsed.query(), &source)
        .unwrap()
        .execute_expression(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                error_messages: Some(&messages),
                ..Default::default()
            },
        );
    assert_eq!(result.err(), Some(EvalError::DivisionByZero.into()));
}
