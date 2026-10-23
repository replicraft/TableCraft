use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_core::values::{ExtendedDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, clock: Option<&dyn Clock>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                clock,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn date_returns_keep_variant_declarations_through_query_scopes() {
    for expression in [
        "DateSerial(1899,12,29)",
        "TimeSerial(-24,0,0)",
        "DateAdd('d',-1,CDate(0))",
        "DateValue(CDate(-1))",
        "-TimeValue(CDate(0.75))",
    ] {
        for sql in [
            format!("SELECT CInt(-32768)\\({expression})"),
            format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT (SELECT CInt(-32768)\\Q.N) FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q GROUP BY Q.N"),
        ] {
            assert_eq!(
                run(&sql, None).unwrap(),
                vec![vec![Value::Int32(32768)]],
                "{sql}"
            );
        }
        let sql = format!("SELECT CInt(-32768)\\CDate({expression})");
        assert_eq!(run(&sql, None), Err(EvalError::Overflow.into()), "{sql}");
    }
    let sql = "SELECT DateValue(NULL)+CInt(1),TimeValue(NULL)+CInt(1)";
    assert_eq!(
        run(sql, None).unwrap(),
        vec![vec![Value::Null, Value::Null]]
    );
    let sql = "SELECT CInt(-32768)\\DateAdd('d',-1,CDate(1/0))";
    assert_eq!(run(sql, None), Err(EvalError::DivisionByZero.into()));
}

struct CountingClock(AtomicUsize);

impl Clock for CountingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        ClockReading::new(ExtendedDateTime::new(1899, 12, 29, 6, 0, 0, 0).unwrap())
    }
}

#[test]
fn clock_declarations_do_not_add_provider_reads() {
    let clock = CountingClock(AtomicUsize::new(0));
    for expression in ["Date()", "Now()", "-Time()*CInt(4)"] {
        let before = clock.0.load(Ordering::SeqCst);
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        assert_eq!(
            run(&sql, Some(&clock)).unwrap(),
            vec![vec![Value::Int32(32768)]]
        );
        assert_eq!(clock.0.load(Ordering::SeqCst), before + 1);
    }
    assert_eq!(
        run("SELECT CInt(-32768)\\Date()", None),
        Err(EvalError::ClockRequired.into())
    );
    let before = clock.0.load(Ordering::SeqCst);
    let sql = "SELECT CInt(-32768)\\(-Timer()/CDbl(21600))";
    assert_eq!(run(sql, Some(&clock)), Err(EvalError::Overflow.into()));
    assert_eq!(clock.0.load(Ordering::SeqCst), before + 1);
}
