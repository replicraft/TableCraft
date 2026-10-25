use super::*;
use std::cell::Cell;
use tablecraft_query::QueryLimits;

#[test]
fn lossy_unicode_aliases_undefined_tokens_and_invalid_raw_sources_are_refused() {
    for text in ["¥", "‾", "−", "😀", "\u{e000}"] {
        let sql = format!("SELECT LenB(StrConv('{text}',128,1041))");
        assert!(run(&sql).is_err(), "{sql}");
    }
    for bytes in [
        vec![0x81],
        vec![0x81, 0x7f],
        vec![0xa0],
        vec![0xfd],
        vec![0xfe],
        vec![0xff],
    ] {
        let input = if bytes.len() == 1 {
            format!("ChrB({})", bytes[0])
        } else {
            let unit = u16::from_le_bytes([bytes[0], bytes[1]]);
            format!("ChrW({unit})")
        };
        let sql = format!("SELECT StrConv({input},64,1041)");
        assert!(run(&sql).is_err(), "{sql}");
    }
    for sql in [
        "SELECT LenB(StrConv(ChrW(55296),128,1041))",
        "SELECT LenB(StrConv(LeftB('A',1),128,1041))",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
    assert_eq!(
        run("SELECT StrConv(ChrB(240)&ChrB(64),64,1041)"),
        Err(QueryError::Evaluation(EvalError::UnrepresentableText))
    );

    for (bytes, text) in [("ChrB(128)", "\u{80}"), ("ChrW(16624)", "\u{e000}")] {
        let sql = format!("SELECT StrConv({bytes},64,1041)");
        assert_eq!(run(&sql).unwrap(), [vec![Value::Text(text.into())]]);
    }
}

#[test]
fn source_and_controls_remain_eager_before_codec_conversion() {
    for sql in [
        "SELECT StrConv(NULL,128,1/0)",
        "SELECT StrConv(IIf(True,'日本',1/0),128,1041)",
        "SELECT StrConv('日本',128+0*(1/0),1041)",
        "SELECT StrConv(ChrW(55296),128,1/0)",
    ] {
        assert_eq!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT StrConv(NULL,128,1041),StrConv(NULL,64,1041)").unwrap(),
        [vec![Value::Null, Value::Null]]
    );
    assert!(run("SELECT StrConv(NULL,128,999999)").is_err());
}

#[test]
fn every_observed_checkpoint_and_cumulative_work_limit_refuses_complete_results() {
    let source = MemorySource::default();
    let syntax = parse_query(
        "SELECT StrConv(StrConv('日本ｶﾞA',128,1041),64,1041)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let checks = Cell::new(0);
    let observe = || {
        checks.set(checks.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&observe),
            ..Default::default()
        },
    )
    .unwrap();
    let total = checks.get();
    assert!(total > 0);
    for stop in 1..=total {
        checks.set(0);
        let cancel = || {
            checks.set(checks.get() + 1);
            checks.get() == stop
        };
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
    for steps in [1, 2, 5] {
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    steps,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(result.is_err(), "steps {steps}");
    }
}
