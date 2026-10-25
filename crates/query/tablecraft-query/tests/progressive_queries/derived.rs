//! Inline continuations retain eager work, layer-local TOP and expression cells.
use super::*;
use tablecraft_expressions::EvalError;

fn stream(sql: &str, source: &Rows, declared: bool) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let prepared = prepare(syntax.query(), source).unwrap();
    let mut rows = Vec::new();
    prepared.execute_progressive(
        source,
        &[],
        ExecutionOptions {
            declared_integral: declared,
            ..Default::default()
        },
        &mut |row| {
            rows.push(row.to_vec());
            Ok(())
        },
    )?;
    Ok(rows)
}

#[test]
fn each_projection_alias_and_top_counts_its_own_stage_outputs() {
    let sql = "SELECT TOP 2 CLng(W),CLng(W+1)
               FROM (SELECT TOP 3 V+CLng(1) AS W
                     FROM (SELECT N AS V,N+10 AS Unused FROM T) A WHERE V>0) B
               WHERE W>=3";
    for declared in [false, true] {
        let source = Rows::new([0, 1, 2, 3, 4]);
        assert_eq!(
            stream(sql, &source, declared).unwrap(),
            [
                vec![Value::Int32(3), Value::Int32(4)],
                vec![Value::Int32(4), Value::Int32(5)],
            ]
        );
        assert_eq!(source.visits.get(), 5);
        assert_eq!(source.scans.get(), 1);

        let source = Rows::new([1, 2, 3, 4]);
        assert!(
            stream(
                "SELECT N FROM (SELECT TOP 2 N FROM T) Q WHERE N>=3",
                &source,
                declared,
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(source.visits.get(), 4);
    }
}

#[test]
fn unused_inner_projections_still_fail_under_outer_false_filters_and_inner_top() {
    let source = Rows::new([1, 2, 3]);
    let filtered = "SELECT V FROM (SELECT N AS V,1/(N-2) AS Unused FROM T) Q WHERE False";
    assert!(matches!(
        stream(filtered, &source, false),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    let mut source = Rows::new([1, 2, 3]);
    source.swallow = true;
    source.eof_error = true;
    let syntax = parse_query(
        "SELECT V FROM (SELECT TOP 1 N AS V,1/(N-2) AS Unused FROM T) Q",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let mut rows = Vec::new();
    let result = prepared.execute_progressive(&source, &[], Default::default(), &mut |row| {
        rows.push(row.to_vec());
        Ok::<(), ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(rows, [vec![Value::Int32(1)]]);
    assert_eq!(source.visits.get(), 3);
}

#[test]
fn raw_and_error_cells_cross_layers_without_stored_or_scalar_publication() {
    let sql = "SELECT LenB(Raw),LenB(Odd),CLng(Code),K
               FROM (SELECT N AS K,ChrW(55296) AS Raw,ChrB(65) AS Odd,CVErr(N) AS Code FROM T) Q";
    for declared in [false, true] {
        let source = Rows::new([7]);
        assert_eq!(
            stream(sql, &source, declared).unwrap(),
            [vec![
                Value::Int32(2),
                Value::Int32(1),
                Value::Int32(7),
                Value::Int32(7)
            ]]
        );
        let source = Rows::new([7]);
        assert_eq!(
            stream(
                "SELECT K FROM (SELECT N AS K,ChrW(55296) AS Raw,CVErr(N) AS Code FROM T) Q",
                &source,
                declared
            )
            .unwrap(),
            [vec![Value::Int32(7)]]
        );
        let source = Rows::new([7]);
        assert!(
            stream(
                "SELECT Raw FROM (SELECT ChrW(55296) AS Raw FROM T) Q",
                &source,
                declared
            )
            .is_err()
        );
    }
}

#[test]
fn existing_global_and_grouped_aggregates_consume_only_delivered_layer_rows() {
    let source = Rows::new([-1, 1, 2, 3]);
    let sql = "SELECT Count(*),Sum(V) FROM (SELECT TOP 2 N AS V FROM T WHERE N>0) Q";
    assert_eq!(
        stream(sql, &source, false).unwrap(),
        [vec![Value::Int64(2), Value::Int64(3)]]
    );
    assert_eq!(source.visits.get(), 4);

    let source = Rows::new([3, 1, 2, 4]);
    let sql = "SELECT CLng(V Mod 2),Count(*)
               FROM (SELECT N AS V,ChrW(55296) AS Unused FROM T) Q GROUP BY V Mod 2";
    assert_eq!(
        stream(sql, &source, false).unwrap(),
        [
            vec![Value::Int32(1), Value::Int64(2)],
            vec![Value::Int32(0), Value::Int64(2)]
        ]
    );
    assert_eq!(source.visits.get(), 4);
}
