use super::*;

#[test]
fn flat_union_all_streams_each_arm_once_in_written_order() {
    let source = Rows::new([1, 2]);
    let syntax = parse_query(
        "SELECT N AS Original FROM T UNION ALL SELECT N+10 FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(plan.output_names().unwrap(), ["Original"]);
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            rows.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(summary.scanned_rows, 4);
    assert_eq!(summary.emitted_rows, 4);
    assert_eq!(source.scans.get(), 2);
    assert_eq!(source.visits.get(), 4);
    assert_eq!(
        rows,
        [1, 2, 11, 12]
            .map(|number| vec![Value::Int64(number)])
            .to_vec()
    );
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        rows
    );
}

#[test]
fn each_union_arm_top_limits_publication_independently_and_still_reaches_eof() {
    let source = Rows::new([1, 2, 3]);
    let syntax = parse_query(
        "SELECT TOP 1 N FROM T UNION ALL SELECT TOP 2 N+10 FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            rows.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        rows,
        [
            vec![Value::Int64(1)],
            vec![Value::Int64(11)],
            vec![Value::Int64(12)]
        ]
    );
    assert_eq!(summary.scanned_rows, 6);
    assert_eq!(summary.emitted_rows, 3);
    assert_eq!(source.visits.get(), 6);
    assert_eq!(source.scans.get(), 2);
}

#[test]
fn every_union_arm_shape_is_admitted_before_any_scan_or_publication() {
    for sql in [
        "SELECT N FROM T UNION SELECT N FROM T",
        "SELECT N FROM T UNION ALL SELECT Max(N) FROM T WHERE False",
        "SELECT N FROM T UNION ALL SELECT N FROM T ORDER BY N",
        "SELECT N FROM T UNION ALL SELECT DISTINCT N FROM T WHERE False",
        "SELECT N FROM T UNION ALL SELECT N FROM (SELECT N FROM T) Q",
        "SELECT N FROM T UNION ALL SELECT 1",
        "SELECT 1 UNION ALL SELECT N FROM T",
    ] {
        let source = Rows::new([1]);
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut emitted = 0;
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            emitted += 1;
            Ok::<_, ()>(())
        });
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Unsupported))),
            "{sql}"
        );
        assert_eq!(emitted, 0, "{sql}");
        assert_eq!(source.scans.get(), 0, "{sql}");
    }
}

#[test]
fn swallowed_sink_failure_never_starts_a_later_union_arm() {
    let mut source = Rows::new([1, 2, 3]);
    source.swallow = true;
    let syntax = parse_query(
        "SELECT N FROM T UNION ALL SELECT N FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut callbacks = 0;
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        callbacks += 1;
        Err("original sink failure")
    });
    assert!(matches!(
        result,
        Err(StreamError::Sink("original sink failure"))
    ));
    assert_eq!(callbacks, 1);
    assert_eq!(source.scans.get(), 1);
    assert_eq!(source.visits.get(), 3);
}
