use super::*;

#[test]
fn three_named_barriers_and_intermediate_row_filters_use_one_physical_cursor() {
    let mut source = Input::fixture();
    source.define("PerGroup", "SELECT G,Count(*) AS N FROM T GROUP BY G");
    source.define(
        "GrandTotals",
        "SELECT Count(*) AS GroupTotal FROM PerGroup WHERE N=2",
    );
    let (summary, actual) = run(
        "SELECT Sum(GroupTotal) FROM GrandTotals",
        &source,
        &[],
        false,
        false,
    )
    .unwrap();
    assert_eq!(actual, [vec![Value::Int64(3)]]);
    assert_eq!(summary.scanned_rows, 6);
    assert_eq!(summary.emitted_rows, 1);
    assert_eq!(source.cursors.get(), 1);
    assert_eq!(source.visits.get(), 6);
}

#[test]
fn remapped_named_parameters_and_prior_output_aliases_stay_in_their_declared_scope() {
    let mut source = Input::fixture();
    source.define("PerGroup", "PARAMETERS [Floor.] LONG; SELECT G,Count(*) AS N,N+CLng(1) AS NextCount FROM T WHERE Id>=[Floor.] GROUP BY G");
    let sql = "PARAMETERS [floor.] LONG; SELECT Sum(NextCount),Count(*) FROM PerGroup";
    for prepare_declared in [false, true] {
        for declared_integral in [false, true] {
            let (_, actual) = run(
                sql,
                &source,
                &[("FLOOR.", Value::Int32(3))],
                prepare_declared,
                declared_integral,
            )
            .unwrap();
            // Remaining group sizes1,1,2; each selected prior alias adds one.
            assert_eq!(actual, [vec![Value::Int64(7), Value::Int64(3)]]);
        }
    }
}

#[test]
fn inner_top_precedes_outer_filter_without_ending_physical_validation() {
    let source = Input::fixture();
    let (summary, actual) = run(
        "SELECT Count(*) FROM (SELECT TOP 1 G,Count(*) AS N FROM T GROUP BY G) Q WHERE G=1",
        &source,
        &[],
        false,
        false,
    )
    .unwrap();
    assert_eq!(actual, [vec![Value::Int64(0)]]);
    assert_eq!(summary.scanned_rows, 6);
    assert_eq!(source.visits.get(), 6);
}

#[test]
fn downstream_false_does_not_suppress_inner_selected_work_but_inner_having_does() {
    let source = Input::fixture();
    let failing = run(
        "SELECT Count(*) FROM (SELECT G,Sum(1/0) AS Bad FROM T GROUP BY G) Q WHERE False",
        &source,
        &[],
        false,
        false,
    );
    assert!(matches!(
        failing,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    let (_, actual) = run(
        "SELECT Count(*) FROM (SELECT G,Sum(1/0) AS Bad FROM T GROUP BY G HAVING False) Q WHERE False",
        &source, &[], false, false,
    ).unwrap();
    assert_eq!(actual, [vec![Value::Int64(0)]]);
}

#[test]
fn paired_inner_group_descriptors_are_selected_by_runtime_profile_without_sampling() {
    let source = Input::fixture();
    for prepare_declared in [false, true] {
        let before = source.cursors.get();
        let sql =
            "SELECT K,VarType(K) FROM (SELECT Id+Id AS K,Count(*) AS N FROM T GROUP BY Id+Id) Q";
        assert!(matches!(
            run(sql, &source, &[], prepare_declared, false),
            Err(StreamError::Query(QueryError::Unsupported))
        ));
        assert_eq!(source.cursors.get(), before);
        let (_, actual) = run(sql, &source, &[], prepare_declared, true).unwrap();
        let expected: Vec<_> = (1..=6)
            .map(|id| vec![Value::Int32(id * 2), Value::Int16(3)])
            .collect();
        assert_eq!(actual, expected);
    }
}
