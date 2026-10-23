use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation, predicates::LikeMode};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn run(sql: &str, declared: bool, mode: Option<LikeMode>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                collation: Some(&Ordinal),
                like_mode: mode,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn like_categories_survive_negation_and_query_composition_in_both_modes() {
    for (mode, pattern) in [(LikeMode::Ansi89, "a*"), (LikeMode::Ansi92, "a%")] {
        for (left, right, variant) in [
            ("'abc'".to_owned(), format!("'{pattern}'"), false),
            ("CVar('abc')".to_owned(), format!("'{pattern}'"), true),
            ("'abc'".to_owned(), format!("CVar('{pattern}')"), true),
        ] {
            for expression in [
                format!("{left} LIKE {right}"),
                format!("Not ({left} NOT LIKE {right})"),
            ] {
                for sql in [
                    format!("SELECT CInt(-32768)\\({expression})"),
                    format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q"),
                    format!(
                        "SELECT (SELECT CInt(-32768)\\Q.N) FROM (SELECT {expression} AS N) AS Q"
                    ),
                    format!(
                        "SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q GROUP BY Q.N"
                    ),
                ] {
                    if variant {
                        assert_eq!(
                            run(&sql, true, Some(mode)).unwrap(),
                            vec![vec![Value::Int32(32768)]],
                            "{sql}"
                        );
                    } else {
                        assert_eq!(
                            run(&sql, true, Some(mode)),
                            Err(EvalError::Overflow.into()),
                            "{sql}"
                        );
                    }
                    assert_eq!(
                        run(&sql, false, Some(mode)).unwrap(),
                        vec![vec![Value::Int32(32768)]],
                        "{sql}"
                    );
                }
            }
        }
    }
}

#[test]
fn null_checks_have_scalar_signatures_independently_of_their_arguments() {
    for expression in [
        "IsNull(CVar(NULL))",
        "CVar(NULL) IS NULL",
        "CVar(1) IS NOT NULL",
        "Count(*) IS NOT NULL",
    ] {
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        assert_eq!(
            run(&sql, true, None),
            Err(EvalError::Overflow.into()),
            "{sql}"
        );
        let sql = format!("SELECT CInt(-32768)\\CVar({expression})");
        assert_eq!(
            run(&sql, true, None).unwrap(),
            vec![vec![Value::Int32(32768)]],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT CInt(-32768)\\IsNull(CVErr(0))", true, None),
        Err(EvalError::DivisionByZero.into())
    );
    assert_eq!(
        run("SELECT IsNull(1/0)", true, None),
        Err(EvalError::DivisionByZero.into())
    );
}

#[test]
fn unknown_like_declarations_and_existing_pattern_failures_are_not_hidden() {
    let sql = "SELECT CInt(-32768)\\(First('a') LIKE 'a')";
    assert_eq!(
        run(sql, true, Some(LikeMode::Ansi89)),
        Err(QueryError::ArithmeticDeclaration)
    );
    let sql = "SELECT CInt(-32768)\\(First('a') LIKE CVar('a'))";
    assert_eq!(
        run(sql, true, Some(LikeMode::Ansi89)).unwrap(),
        vec![vec![Value::Int32(32768)]]
    );
    assert_eq!(
        run(
            "SELECT (CVar(NULL) LIKE 'a')+CInt(1)",
            true,
            Some(LikeMode::Ansi89)
        )
        .unwrap(),
        vec![vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT CInt(-32768)\\('a' LIKE 'a')", true, None),
        Err(EvalError::LikeModeRequired.into())
    );
    assert_eq!(
        run(
            "SELECT CInt(-32768)\\(CVar('a') LIKE '[')",
            true,
            Some(LikeMode::Ansi89)
        ),
        Err(EvalError::InvalidPattern.into())
    );
    assert_eq!(
        run(
            "SELECT CVar('a') LIKE CVar(CStr(1/0))",
            true,
            Some(LikeMode::Ansi89)
        ),
        Err(EvalError::DivisionByZero.into())
    );
}

struct CountingCollation(Cell<usize>);

impl Collation for CountingCollation {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        self.0.set(self.0.get() + 1);
        Ok(left.cmp(right))
    }
}

#[test]
fn declaration_tracking_does_not_repeat_the_matcher() {
    let source = MemorySource::default();
    let collation = CountingCollation(Cell::new(0));
    let parsed = parse_query(
        "SELECT CInt(-32768)\\(CVar('a') LIKE 'a')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let options = ExecutionOptions {
        declared_integral: true,
        collation: Some(&collation),
        like_mode: Some(LikeMode::Ansi89),
        ..Default::default()
    };
    assert_eq!(
        plan.execute(&source, &[], options).unwrap().rows(),
        &[vec![Value::Int32(32768)]]
    );
    assert_eq!(collation.0.get(), 1);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| true),
                ..options
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
}
