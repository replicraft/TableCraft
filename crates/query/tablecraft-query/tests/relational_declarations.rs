use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation, functions::math::random::RndState};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn comparison_categories_survive_grouped_derived_and_correlated_outputs() {
    for (operator, left, right) in [
        ("=", 1, 1),
        ("<>", 1, 2),
        ("<", 1, 2),
        ("<=", 1, 1),
        (">", 2, 1),
        (">=", 1, 1),
    ] {
        for (left, right, variant) in [
            (format!("CInt({left})"), format!("CInt({right})"), false),
            (
                format!("CVar(CInt({left}))"),
                format!("CInt({right})"),
                true,
            ),
            (
                format!("CInt({left})"),
                format!("CVar(CInt({right}))"),
                true,
            ),
        ] {
            let expression = format!("{left} {operator} {right}");
            for sql in [
                format!("SELECT CInt(-32768)\\({expression})"),
                format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q"),
                format!("SELECT (SELECT CInt(-32768)\\Q.N) FROM (SELECT {expression} AS N) AS Q"),
                format!(
                    "SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q GROUP BY Q.N"
                ),
            ] {
                if variant {
                    assert_eq!(
                        run(&sql, true).unwrap(),
                        vec![vec![Value::Int32(32768)]],
                        "{sql}"
                    );
                } else {
                    assert_eq!(run(&sql, true), Err(EvalError::Overflow.into()), "{sql}");
                }
                assert_eq!(
                    run(&sql, false).unwrap(),
                    vec![vec![Value::Int32(32768)]],
                    "{sql}"
                );
            }
        }
    }
}

#[test]
fn nulls_error_data_and_unknown_categories_keep_their_boundaries() {
    assert_eq!(
        run("SELECT CInt(-32768)\\(NULL=CInt(1))", true).unwrap(),
        vec![vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT CInt(-32768)\\(CInt(1)=CInt(2))", true),
        Err(EvalError::DivisionByZero.into())
    );
    assert_eq!(
        run("SELECT CInt(-32768)\\(Count(*)=CInt(1))", true),
        Err(QueryError::ArithmeticDeclaration)
    );
    assert_eq!(
        run("SELECT CInt(-32768)\\(Count(*)=CVar(CInt(1)))", true).unwrap(),
        vec![vec![Value::Int32(32768)]]
    );
    let sql = "SELECT CInt(-32768)\\(CVErr(5)=CVErr(5))";
    assert_eq!(run(sql, true).unwrap(), vec![vec![Value::Int32(32768)]]);
    let sql = "SELECT CInt(-32768)\\CBool(CVErr(5)=CVErr(5))";
    assert_eq!(run(sql, true), Err(EvalError::Overflow.into()));
    assert_eq!(
        run("SELECT CInt(-32768)\\(CVErr(5)=CInt(5))", true),
        Err(EvalError::TypeMismatch.into())
    );
    for sql in [
        "SELECT CInt(-32768)\\(CVar(1)=1/0)",
        "SELECT CInt(-32768)\\(1/0=CVar(1))",
    ] {
        assert_eq!(
            run(sql, true),
            Err(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
}

struct CountingCollation(Cell<usize>);

impl Collation for CountingCollation {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        self.0.set(self.0.get() + 1);
        Ok(left.cmp(right))
    }
}

#[test]
fn declarations_use_the_existing_comparator_without_repeating_operands() {
    let source = MemorySource::default();
    let collation = CountingCollation(Cell::new(0));
    let parsed = parse_query("SELECT CInt(-32768)\\(CVar('a')='a')", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                collation: Some(&collation),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Int32(32768)]]);
    assert_eq!(collation.0.get(), 1);

    let random = RndState::new(7).unwrap();
    let reference = RndState::new(7).unwrap();
    reference.apply(None).unwrap();
    reference.apply(None).unwrap();
    let parsed = parse_query("SELECT CVar(Rnd())=Rnd()", Default::default()).unwrap();
    prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&random),
                declared_integral: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(random.apply(None).unwrap(), reference.apply(None).unwrap());
}
