//! Scalar String/Variant conversion is distinct from two-Variant category order.
use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                number_format: Some(NumberFormat::Invariant),
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn all_six_comparisons_format_known_variant_numeric_in_both_orders() {
    for carrier in [
        "CByte", "CInt", "CLng", "CLngLng", "CSng", "CDbl", "CCur", "CDec",
    ] {
        for text in ["02", "10", "bad", "2"] {
            for reverse in [false, true] {
                let (left, right) = if reverse { ("2", text) } else { (text, "2") };
                for (operator, expected) in [
                    ("=", left == right),
                    ("<>", left != right),
                    ("<", left < right),
                    ("<=", left <= right),
                    (">", left > right),
                    (">=", left >= right),
                ] {
                    let (left, right) = if reverse {
                        (format!("CVar({carrier}(2))"), format!("'{text}'"))
                    } else {
                        (format!("'{text}'"), format!("CVar({carrier}(2))"))
                    };
                    let sql = format!("SELECT {left}{operator}{right}");
                    assert_eq!(
                        run(&sql, true).unwrap(),
                        [vec![Value::Boolean(expected)]],
                        "{sql}"
                    );
                }
            }
        }
    }
    assert_eq!(
        run("SELECT '02'=CVar(2),'10'<CVar(2)", false).unwrap(),
        [vec![Value::Boolean(true), Value::Boolean(false)]]
    );
}

#[test]
fn categories_keep_numeric_scalar_and_both_variant_paths_distinct() {
    assert_eq!(
        run(
            "SELECT CVar('02')=CVar(2),CVar('10')<CVar(2),CVar(2)<'bad'",
            true
        )
        .unwrap(),
        [vec![
            Value::Boolean(false),
            Value::Boolean(false),
            Value::Boolean(true)
        ]]
    );
    assert_eq!(
        run(
            "SELECT CLng(2)=CVar('02'),CVar('02')=CLng(2),'02'=CLng(2)",
            true
        )
        .unwrap(),
        [vec![
            Value::Boolean(true),
            Value::Boolean(true),
            Value::Boolean(true)
        ]]
    );
    assert_eq!(
        run(
            "SELECT 'True'=CVar(True),'False'=CVar(False),'x'=CVar(NULL)",
            true
        )
        .unwrap(),
        [vec![
            Value::Boolean(true),
            Value::Boolean(true),
            Value::Null
        ]]
    );
}

#[test]
fn same_dispatch_reaches_derived_grouped_membership_quantified_and_correlated_predicates() {
    for sql in [
        "SELECT '02'=V FROM (SELECT CVar(CLng(2)) AS V) AS Q",
        "SELECT '02'=CVar(First(V)) FROM (SELECT CLng(2) AS V) AS Q",
        "SELECT '02' IN (CVar(2),CVar(3))",
        "SELECT '02' BETWEEN CVar(2) AND CVar(3)",
        "SELECT '02'=ANY(SELECT CVar(2) AS V)",
        "SELECT (SELECT '02'=Q.V) FROM (SELECT CVar(2) AS V) AS Q",
    ] {
        assert_eq!(
            run(sql, true).unwrap(),
            [vec![Value::Boolean(false)]],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT '10'<CVar(2) UNION ALL SELECT False", true).unwrap(),
        [vec![Value::Boolean(true)], vec![Value::Boolean(false)]]
    );
}

#[test]
fn eager_expression_errors_and_unsupported_states_do_not_format_or_disappear() {
    for (sql, expected) in [
        ("SELECT 'x'=CVErr(1)", EvalError::TypeMismatch),
        ("SELECT 'x'=CVar(1/0)", EvalError::DivisionByZero),
        (
            "SELECT CVar(1/0)=CStr(CVErr(-1))",
            EvalError::DivisionByZero,
        ),
        ("SELECT '02' IN (CVar(2),1/0)", EvalError::DivisionByZero),
        (
            "SELECT '02' BETWEEN CVar(2) AND 1/0",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT CStr(ChrW(55296))=CVar(2)",
            EvalError::UnrepresentableText,
        ),
    ] {
        assert_eq!(
            run(sql, true).err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT CStr(ChrW(55296))=CVar(NULL)", true).unwrap(),
        [vec![Value::Null]]
    );
}

#[test]
fn unknown_categories_refuse_only_declaration_dependent_comparisons() {
    for sql in [
        "SELECT '02'=First(CLng(2)) FROM (SELECT 1 AS N) AS Q",
        "SELECT First('02')=CVar(CLng(2)) FROM (SELECT 1 AS N) AS Q",
        "SELECT First('02')=First(CLng(2)) FROM (SELECT 1 AS N) AS Q",
        "SELECT CVar('02')=First(CLng(2)) FROM (SELECT 1 AS N) AS Q",
        "SELECT 'True'=First(True) FROM (SELECT 1 AS N) AS Q",
        "SELECT First('True')=CVar(True) FROM (SELECT 1 AS N) AS Q",
    ] {
        assert_eq!(
            run(sql, true).err(),
            Some(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
    for sql in [
        "SELECT First('02')=CLng(2) FROM (SELECT 1 AS N) AS Q",
        "SELECT First('02')='02' FROM (SELECT 1 AS N) AS Q",
        "SELECT CVar('True')=First(True) FROM (SELECT 1 AS N) AS Q",
    ] {
        assert_eq!(
            run(sql, true).unwrap(),
            [vec![Value::Boolean(true)]],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT '02'=First(NULL) FROM (SELECT 1 AS N) AS Q", true).unwrap(),
        [vec![Value::Null]]
    );
    assert_eq!(
        run(
            "SELECT '02'=First(CVErr(1)) FROM (SELECT 1 AS N) AS Q",
            true
        )
        .err(),
        Some(EvalError::TypeMismatch.into())
    );
}

#[path = "declared_string_comparisons/policies.rs"]
mod policies;
