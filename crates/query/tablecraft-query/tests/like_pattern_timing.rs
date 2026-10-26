use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LikeProfile},
    predicates::LikeMode,
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn run(
    sql: &str,
    mode: LikeMode,
    declared: bool,
    provider: &dyn Collation,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source)?;
    Ok(plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                like_mode: Some(mode),
                collation: Some(provider),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn projections_and_predicates_follow_element_error_frontiers_in_both_modes() {
    for declared in [false, true] {
        for (mode, star) in [(LikeMode::Ansi89, "*"), (LikeMode::Ansi92, "%")] {
            for sql in [
                "SELECT 'x' LIKE 'a[','x' NOT LIKE 'a[','a' LIKE 'a['".to_owned(),
                format!("SELECT 'x' LIKE 'a{star}[','x' NOT LIKE 'a{star}[','' LIKE '['"),
                "SELECT 'x' LIKE 'a[z-a]','x' NOT LIKE 'a[z-a]','' LIKE '[z-a]'".to_owned(),
            ] {
                assert_eq!(
                    run(&sql, mode, declared, &Ordinal).unwrap(),
                    [vec![
                        Value::Boolean(false),
                        Value::Boolean(true),
                        Value::Boolean(false)
                    ]],
                    "{sql}"
                );
            }
            for sql in [
                "SELECT 'ab' LIKE 'a['".to_owned(),
                "SELECT 'ab' NOT LIKE 'a[z-a]'".to_owned(),
                format!("SELECT 'x' LIKE '{star}a['"),
                format!("SELECT '' NOT LIKE '{star}['"),
            ] {
                assert_eq!(
                    run(&sql, mode, declared, &Ordinal),
                    Err(EvalError::InvalidPattern.into()),
                    "{sql}"
                );
            }
            let sql = "SELECT Q.S FROM (SELECT 'x' AS S) Q WHERE Q.S NOT LIKE 'a['";
            assert_eq!(
                run(sql, mode, declared, &Ordinal).unwrap(),
                [vec![Value::Text("x".into())]]
            );
        }
    }
}

struct Units;

impl Collation for Units {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }

    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }

    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }

    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        Ok(units.len() == 1)
    }
}

#[test]
fn retained_raw_units_keep_matching_timing_without_unicode_publication() {
    for (mode, one) in [(LikeMode::Ansi89, "?"), (LikeMode::Ansi92, "_")] {
        let sql = format!(
            "SELECT ChrW(55296) LIKE 'a[',ChrW(55296) NOT LIKE '{one}[','x' LIKE ChrW(55296)&'['"
        );
        assert_eq!(
            run(&sql, mode, true, &Units).unwrap(),
            [vec![
                Value::Boolean(false),
                Value::Boolean(true),
                Value::Boolean(false)
            ]]
        );
    }
}

#[test]
fn null_remains_before_matching_but_written_operands_remain_eager() {
    for mode in [LikeMode::Ansi89, LikeMode::Ansi92] {
        assert_eq!(
            run(
                "SELECT NULL LIKE '[',NULL NOT LIKE '[z-a]',NULL LIKE ChrB(65)",
                mode,
                true,
                &Ordinal
            )
            .unwrap(),
            [vec![Value::Null, Value::Null, Value::Null]]
        );
        for sql in [
            "SELECT 'x' LIKE CStr(1/0)",
            "SELECT NULL LIKE CStr(1/0)",
            "SELECT CStr(1/0) NOT LIKE 'a['",
        ] {
            assert_eq!(
                run(sql, mode, true, &Ordinal),
                Err(EvalError::DivisionByZero.into()),
                "{sql}"
            );
        }
    }
}
