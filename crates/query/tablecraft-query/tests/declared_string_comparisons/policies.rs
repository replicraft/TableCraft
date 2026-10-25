use super::*;
use std::cell::RefCell;
use tablecraft_query::{
    Source,
    source::{CollationResolver, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};

struct Fields {
    columns: Vec<Column>,
    policies: Vec<Option<TextCollationId>>,
}
impl Fields {
    fn new(first_policy: u64) -> Self {
        Self {
            columns: vec![
                Column::new("A".into(), ScalarType::Text, false).unwrap(),
                Column::new("B".into(), ScalarType::Text, false).unwrap(),
            ],
            policies: vec![
                Some(TextCollationId::from_host(first_policy)),
                Some(TextCollationId::from_host(2)),
            ],
        }
    }
}
impl Source for Fields {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn text_collations(&self, _: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&self.policies)
    }

    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        visitor(&[Value::Text("02".into()), Value::Text("2".into())])
    }
}
struct Resolver(RefCell<Vec<u64>>);
impl CollationResolver for Resolver {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.0.borrow_mut().push(id.host_key());
        match id.host_key() {
            1 | 2 => Ok(&Ordinal),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}

#[test]
fn only_actual_strings_contribute_comparison_authority() {
    let source = Fields::new(1);
    let resolver = Resolver(RefCell::new(Vec::new()));
    for sql in [
        "SELECT A<CVar(2) FROM T",
        "SELECT A<CVar(CLng(B)) FROM T",
        "SELECT A<CVar(CLng(IIf(True,B,A))) FROM T",
    ] {
        resolver.0.borrow_mut().clear();
        let syntax = parse_query(sql, Default::default()).unwrap();
        let result = prepare(syntax.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    number_format: Some(NumberFormat::Invariant),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Boolean(true)]], "{sql}");
        assert_eq!(*resolver.0.borrow(), [1]);
    }
    resolver.0.borrow_mut().clear();
    let syntax = parse_query("SELECT A=CVar(B) FROM T", Default::default()).unwrap();
    assert_eq!(
        prepare(syntax.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::CollationConflict)
    );
    assert!(resolver.0.borrow().is_empty());
}

#[test]
fn conversion_failures_null_and_error_do_not_activate_a_field_provider() {
    let source = Fields::new(999);
    let resolver = Resolver(RefCell::new(Vec::new()));
    for (expression, expected) in [
        ("A=CVar(2)", Some(EvalError::NumberFormatRequired)),
        ("A=CVar(CDate(0))", Some(EvalError::DateFormatRequired)),
        ("A=CVErr(1)", Some(EvalError::TypeMismatch)),
        ("A=CVar(NULL)", None),
    ] {
        let syntax =
            parse_query(&format!("SELECT {expression} FROM T"), Default::default()).unwrap();
        let result = prepare(syntax.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        );
        if let Some(expected) = expected {
            assert_eq!(result.err(), Some(expected.into()), "{expression}");
        } else {
            assert_eq!(result.unwrap().rows(), [vec![Value::Null]]);
        }
        assert!(resolver.0.borrow().is_empty(), "{expression}");
    }
}
