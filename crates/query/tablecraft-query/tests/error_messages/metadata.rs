use super::{CountedMessages, Messages, Ordinal};
use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        error_messages::{ErrorMessageCode, ErrorMessages},
    },
};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare, source::MemorySource,
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

fn text_type(nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(ScalarType::Text),
        nullable,
    }
}

fn options(messages: Option<&dyn ErrorMessages>) -> ExecutionOptions<'_> {
    ExecutionOptions {
        error_messages: messages,
        number_format: Some(NumberFormat::Invariant),
        collation: Some(&Ordinal),
        ..Default::default()
    }
}

#[test]
fn numbered_message_text_metadata_supplies_union_conversion_without_catalog_lookup() {
    let source = MemorySource::default();
    for function in ["Error", "Error$"] {
        for operation in ["UNION", "UNION ALL"] {
            for reverse in [false, true] {
                let message = format!("SELECT {function}(0) AS V");
                let sql = if reverse {
                    format!("SELECT 1 AS V {operation} {message}")
                } else {
                    format!("{message} {operation} SELECT 1")
                };
                let parsed = parse_query(&sql, Default::default()).unwrap();
                let plan = prepare(parsed.query(), &source).unwrap();
                assert_eq!(plan.output_types().unwrap(), [text_type(false)]);

                let result = plan.execute(&source, &[], options(None)).unwrap();
                let mut expected = vec![
                    vec![Value::Text(String::new())],
                    vec![Value::Text("1".into())],
                ];
                if reverse {
                    expected.reverse();
                }
                assert_eq!(result.rows(), expected, "{sql}");
                assert_eq!(result.output_types(), [text_type(false)]);
            }
        }
    }
}

struct Named {
    query: Query,
}

impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("Messages").then_some(&self.query))
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Err(QueryError::Source)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("source-free named metadata and execution must not scan tables")
    }
}

#[test]
fn text_type_survives_named_derived_aggregate_empty_and_provider_rebinding_paths() {
    for function in ["Error", "Error$"] {
        let source = Named {
            query: parse_query(
                &format!("SELECT {function}(11) AS Message"),
                Default::default(),
            )
            .unwrap()
            .into_query(),
        };
        for (sql, nullable) in [
            ("SELECT Message FROM Messages UNION ALL SELECT 1", false),
            (
                "SELECT D.Message FROM (SELECT Message FROM Messages) AS D UNION ALL SELECT 1",
                false,
            ),
            (
                "SELECT First(Message) FROM Messages UNION ALL SELECT 1",
                true,
            ),
        ] {
            let messages = CountedMessages(AtomicUsize::new(0));
            let parsed = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            assert_eq!(plan.output_types().unwrap(), [text_type(nullable)]);
            assert_eq!(messages.0.load(Ordering::Relaxed), 0);

            for message in ["selected", "different"] {
                let provider = Messages(message);
                let result = plan
                    .execute(&source, &[], options(Some(&provider)))
                    .unwrap();
                assert_eq!(
                    result.rows(),
                    [
                        vec![Value::Text(message.into())],
                        vec![Value::Text("1".into())]
                    ]
                );
                assert_eq!(result.output_types(), [text_type(nullable)]);
            }
        }

        let sql = format!("SELECT {function}(11) AS Message WHERE False UNION ALL SELECT 1");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [text_type(false)]);
        assert_eq!(
            plan.execute(&source, &[], options(None)).unwrap().rows(),
            [vec![Value::Text("1".into())]]
        );
    }
}

#[test]
fn text_numeric_union_converts_before_duplicate_removal_and_keeps_format_policy_explicit() {
    let source = MemorySource::default();
    let messages = Messages("1");
    for function in ["Error", "Error$"] {
        let sql = format!("SELECT {function}(11) UNION SELECT 1");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], options(Some(&messages)))
                .unwrap()
                .rows(),
            [vec![Value::Text("1".into())]]
        );
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: None,
                    ..options(Some(&messages))
                }
            )
            .err(),
            Some(EvalError::NumberFormatRequired.into())
        );
    }
}

struct FailedMessages;

impl ErrorMessages for FailedMessages {
    fn message(&self, _: ErrorMessageCode) -> Result<&str, EvalError> {
        Err(EvalError::ErrorCatalogFailed)
    }
}

#[test]
fn successful_text_guarantee_does_not_suppress_catalog_argument_or_late_arm_errors() {
    let source = MemorySource::default();
    for function in ["Error", "Error$"] {
        let sql = format!("SELECT {function}(11) UNION ALL SELECT 1");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], options(None)).err(),
            Some(EvalError::ErrorCatalogRequired.into())
        );
        assert_eq!(
            plan.execute(&source, &[], options(Some(&FailedMessages)))
                .err(),
            Some(EvalError::ErrorCatalogFailed.into())
        );

        let sql = format!("SELECT {function}(0) UNION ALL SELECT 1/0");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], options(None))
                .err(),
            Some(EvalError::DivisionByZero.into())
        );
        let sql = format!("SELECT {function}(NULL) UNION ALL SELECT 1");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], options(None))
                .err(),
            Some(EvalError::InvalidNullConversion.into())
        );
    }
}
