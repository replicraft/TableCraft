use std::cmp::Ordering;
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
