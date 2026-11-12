//! VALUES uses the mutation call policy while retaining a source-free environment.
use super::{Target, budget, query};
use crate::{
    InsertError, MutationOptions, QueryError, local_insert_table, plan_insert_with_options,
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, dates::parsing::DateTextFormat},
};

#[test]
fn fixed_math_text_and_date_calls_stage_exact_carriers_before_one_finish() {
    for (sql, number, date, expected) in [
        (
            "INSERT INTO Items VALUES(CLng(Sqr(81))+DatePart('m',DateAdd('m',1,#2026-01-31#)),CStr(DateSerial(2026,10,20)))",
            "invariant",
            "iso",
            [Value::Int32(11), Value::Text("2026-10-20".into())],
        ),
        (
            "INSERT INTO Items VALUES(CLng('1.234'),CStr(1.25))",
            "de-de",
            "de-de",
            [Value::Int32(1234), Value::Text("1,25".into())],
        ),
        (
            "INSERT INTO Items VALUES(Day(CDate('12/31/2026')),Left('alpha',2))",
            "en-us",
            "en-us",
            [Value::Int32(31), Value::Text("al".into())],
        ),
        (
            "INSERT INTO Items VALUES(DatePart('ww',#2024-01-07#,,2),NULL)",
            "invariant",
            "iso",
            [Value::Int32(2), Value::Null],
        ),
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        let actual = plan_insert_with_options(
            Target::ordinary(&calls),
            parsed.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                number_format: NumberFormat::resolve(number),
                date_format: DateTextFormat::resolve(date),
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn explicit_declared_values_do_not_become_source_fields_or_sql() {
    let parsed = query(
        "PARAMETERS [Days] LONG; INSERT INTO Items VALUES(DateDiff('d',#2026-01-01#,DateAdd('d',[Days],#2026-01-01#)),CStr(DateAdd('d',[Days],#2026-01-01#)))",
    );
    let parameters = [("Days", Value::Int32(3))];
    let calls = Cell::new(0);
    let actual = plan_insert_with_options(
        Target::ordinary(&calls),
        parsed.query(),
        &mut budget(),
        &mut budget(),
        MutationOptions {
            parameters: Some(&parameters),
            date_format: DateTextFormat::resolve("iso"),
            ..Default::default()
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(actual, [Value::Int32(3), Value::Text("2026-01-04".into())]);
    assert_eq!(calls.get(), 1);
}

#[test]
fn eager_errors_and_missing_providers_never_call_the_backend() {
    for (sql, expected) in [
        (
            "INSERT INTO Items VALUES(IIf(TRUE,CLng(7),CLng(1/0)),NULL)",
            EvalError::DivisionByZero,
        ),
        (
            "INSERT INTO Items VALUES(Nz(3,CLng(1/0)),NULL)",
            EvalError::DivisionByZero,
        ),
        (
            "INSERT INTO Items(Name,ID) VALUES('complete first value',CLng(Log(0)))",
            EvalError::Domain,
        ),
        (
            "INSERT INTO Items VALUES(1,CStr(1.25))",
            EvalError::NumberFormatRequired,
        ),
        (
            "INSERT INTO Items VALUES(Day(CDate('12/31/2026')),NULL)",
            EvalError::DateFormatRequired,
        ),
        (
            "INSERT INTO Items VALUES(1,CStr(NULL))",
            EvalError::InvalidNullConversion,
        ),
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        let actual = plan_insert_with_options(
            Target::ordinary(&calls),
            parsed.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions::default(),
            &|| false,
        );
        assert!(
            matches!(actual, Err(InsertError::Query(QueryError::Evaluation(error))) if error == expected)
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn runtime_qualified_source_and_malformed_calls_refuse_in_raw_preflight() {
    for expression in [
        "Date()",
        "Now()",
        "Rnd()",
        "Eval('1')",
        "DCount('*','Items')",
        "[Module].Abs(2)",
        "UserFunction(2)",
        "[Array](1,2)",
        "CVDate('2026-01-01')",
        "Sqr(1,2)",
        "DatePart('m')",
        "DateAdd('d',,1)",
        "Left('alpha',*)",
        "Abs(ID)",
    ] {
        let sql = format!("INSERT INTO Items VALUES({expression},NULL)");
        let parsed = tablecraft_sql::parse_query(&sql, tablecraft_sql::ParseLimits::default())
            .unwrap_or_else(|error| panic!("refusal fixture {expression} must parse: {error:?}"));
        assert!(matches!(
            local_insert_table(parsed.query(), false, false, &mut budget(), &|| false),
            Err(QueryError::Unsupported)
        ));
        let calls = Cell::new(0);
        assert!(matches!(
            plan_insert_with_options(
                Target::ordinary(&calls),
                parsed.query(),
                &mut budget(),
                &mut budget(),
                MutationOptions::default(),
                &|| false,
            ),
            Err(InsertError::Query(QueryError::Unsupported))
        ));
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn admitted_calls_remain_cancellable_and_resource_bounded_before_finish() {
    let parsed = query("INSERT INTO Items VALUES(CLng(Sqr(81)),Trim(' paid '))");
    let calls = Cell::new(0);
    let actual = plan_insert_with_options(
        Target::ordinary(&calls),
        parsed.query(),
        &mut ResourceBudget::new(0, 0, 0),
        &mut budget(),
        MutationOptions::default(),
        &|| false,
    );
    assert!(matches!(
        actual,
        Err(InsertError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(calls.get(), 0);

    let actual = plan_insert_with_options(
        Target::ordinary(&calls),
        parsed.query(),
        &mut budget(),
        &mut budget(),
        MutationOptions::default(),
        &|| true,
    );
    assert!(matches!(
        actual,
        Err(InsertError::Query(QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
}
