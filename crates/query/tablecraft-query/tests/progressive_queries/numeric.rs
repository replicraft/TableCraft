//! Numeric aggregates retain their kernels while changing the explicit scan phase.
use super::*;
use tablecraft_core::values::Double;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};

fn collect(sql: &str, source: &Rows, options: ExecutionOptions<'_>) -> Vec<Vec<Value>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let mut output = Vec::new();
    let summary = plan
        .execute_progressive(source, &[], options, &mut |row| {
            assert_eq!(source.visits.get(), source.rows.len());
            output.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(summary.scanned_rows, source.rows.len() as u64);
    assert_eq!(summary.emitted_rows, 1);
    output
}

#[test]
fn numeric_states_finish_before_alias_projection_in_both_profiles() {
    let sql = "SELECT Sum(N) AS Total,Avg(N) AS Mean,Count(*) AS Population,
               Total+Population AS Adjusted,Round(Mean,Count(NULL)) AS Rounded FROM T";
    for declared_integral in [false, true] {
        let source = Rows::new([1, 2, 3]);
        let rows = collect(
            sql,
            &source,
            ExecutionOptions {
                declared_integral,
                ..Default::default()
            },
        );
        assert_eq!(
            rows,
            [vec![
                Value::Int64(6),
                Value::Double(Double::from_float(2.0)),
                Value::Int64(3),
                Value::Int64(9),
                Value::Double(Double::from_float(2.0)),
            ]]
        );
        assert_eq!(source.scans.get(), 1);
    }
}

#[test]
fn exact_floating_totals_preserve_small_terms_between_large_cancellation() {
    let source = Rows::new([1, 2, 3]);
    let operand = "Choose(N,CDbl(1e16),CDbl(1),CDbl(-1e16))";
    let sql = format!("SELECT Sum({operand}),Avg({operand}) FROM T");
    let rows = collect(&sql, &source, Default::default());
    assert_eq!(
        rows,
        [vec![
            Value::Double(Double::from_float(1.0)),
            Value::Double(Double::from_float(1.0 / 3.0))
        ]]
    );
}

#[test]
fn operand_errors_follow_row_then_written_occurrence_order() {
    let sql = "SELECT Sum(1/(3-N)),Avg(CByte(256/N)) FROM T";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let source = Rows::new([1, 2, 3]);
    let plan = prepare(syntax.query(), &source).unwrap();
    let buffered = plan.execute(&source, &[], Default::default());
    assert!(matches!(
        buffered,
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));

    let mut emitted = 0;
    let progressive = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        emitted += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(
        progressive,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::Overflow
        )))
    ));
    assert_eq!(source.visits.get(), 1);
    assert_eq!(emitted, 0);
}

#[test]
fn decimal_finish_failure_waits_for_eof_and_preserves_only_prior_arm_output() {
    let sql = "SELECT CLngLng(N) AS V FROM T WHERE N=1
               UNION ALL SELECT Sum(CDec('79228162514264337593543950335')) FROM T";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let source = Rows::new([1, 2]);
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut output = Vec::new();
    let result = plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            number_format: Some(NumberFormat::Invariant),
            ..Default::default()
        },
        &mut |row| {
            output.push(row.to_vec());
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::Overflow
        )))
    ));
    assert_eq!(source.scans.get(), 2);
    assert_eq!(source.visits.get(), 4);
    assert_eq!(output.len(), 1);
    // The first arm is promoted through the ordinary UNION Decimal descriptor.
    let Value::Decimal(value) = output[0][0] else {
        panic!("expected the retained ordinary Decimal UNION carrier");
    };
    assert_eq!(value.coefficient(), 10_i128.pow(u32::from(value.scale())));
}
