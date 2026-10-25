use super::*;

#[test]
fn named_and_inline_chains_preserve_rows_without_snapshot_or_hidden_field_reads() {
    let source = Inputs::new(
        10,
        &[
            ("Base", "SELECT Id,Score FROM Data WHERE Id Mod 2=0"),
            (
                "StageTwo",
                "SELECT Id AS K,CLng(Id+1) AS NextValue,Score FROM Base WHERE Id<=8",
            ),
            ("Third", "SELECT K,NextValue,Score FROM StageTwo WHERE K>=4"),
        ],
    );
    let inline = "SELECT Q.K,CLng(Q.NextValue*2),Q.Score FROM
        (SELECT K,NextValue,Score FROM
          (SELECT Id AS K,CLng(Id+1) AS NextValue,Score FROM
            (SELECT Id,Score FROM Data) A WHERE Id Mod 2=0 AND Id<=8) B
          WHERE K>=4) Q";
    for declared_integral in [false, true] {
        for sql in [
            "SELECT Q.K,CLng(Q.NextValue*2),Q.Score FROM Third Q",
            inline,
        ] {
            let (summary, rows) = run(
                sql,
                &source,
                &[],
                ExecutionOptions {
                    declared_integral,
                    ..Default::default()
                },
            )
            .unwrap();
            let expected: Vec<_> = [4, 6, 8]
                .into_iter()
                .map(|id| {
                    vec![
                        Value::Int32(id),
                        Value::Int32((id + 1) * 2),
                        Value::Int32(id * 3),
                    ]
                })
                .collect();
            assert_eq!(rows, expected);
            assert_eq!(summary.scanned_rows, 10);
            assert_eq!(summary.emitted_rows, 3);
        }
    }
    assert!(
        source
            .projections
            .borrow()
            .iter()
            .all(|indices| indices == &[1, 2])
    );
    assert_eq!(*source.scans.borrow(), ["Data", "Data", "Data", "Data"]);
}

#[test]
fn aliases_and_scoped_typed_parameters_survive_both_preparation_and_execution_profiles() {
    let source = Inputs::new(
        7,
        &[
            (
                "A",
                "PARAMETERS [Floor.] LONG; SELECT Id AS X,CLng(Id+[Floor.]) AS Y FROM Data WHERE Id>=[Floor.]",
            ),
            (
                "B",
                "PARAMETERS [floor.] LONG; SELECT X,CLng(Y+[Floor.]) AS Z FROM A WHERE X<=5",
            ),
        ],
    );
    let syntax = parse_query(
        "PARAMETERS [Floor.] LONG; SELECT X,Z FROM B",
        Default::default(),
    )
    .unwrap();
    for prepare_declared in [false, true] {
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                declared_arithmetic: prepare_declared,
                ..Default::default()
            },
        )
        .unwrap();
        for declared_integral in [false, true] {
            for floor in [2, 3] {
                let mut rows = Vec::new();
                let summary = plan
                    .execute_progressive(
                        &source,
                        &[("FLOOR.", Value::Int32(floor))],
                        ExecutionOptions {
                            declared_integral,
                            ..Default::default()
                        },
                        &mut |row| {
                            rows.push(row.to_vec());
                            Ok::<_, ()>(())
                        },
                    )
                    .unwrap();
                assert_eq!(
                    rows,
                    (floor..=5)
                        .map(|id| vec![Value::Int32(id), Value::Int32(id + 2 * floor)])
                        .collect::<Vec<_>>()
                );
                assert_eq!(summary.scanned_rows, 7);
            }
        }
    }
}

#[test]
fn raw_and_error_intermediate_cells_are_not_published_or_coerced_before_outer_consumers() {
    let source = Inputs::new(
        3,
        &[(
            "RawView",
            "SELECT Id,ChrW(55296)&Label AS R,CVErr(7) AS E FROM Data",
        )],
    );
    let (_, rows) = run(
        "SELECT Id,LenB(R),CLng(AscW(R)),CLng(E) FROM RawView",
        &source,
        &[],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        rows,
        (1..=3)
            .map(|id| vec![
                Value::Int32(id),
                Value::Int32(12),
                Value::Int32(-10240),
                Value::Int32(7)
            ])
            .collect::<Vec<_>>()
    );
    assert_eq!(
        run(
            "SELECT Count(*) FROM RawView",
            &source,
            &[],
            Default::default()
        )
        .unwrap()
        .1,
        [vec![Value::Int64(3)]]
    );
    assert!(matches!(
        run("SELECT R FROM RawView", &source, &[], Default::default()),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::UnrepresentableText
        )))
    ));
}

#[test]
fn global_and_grouped_consumers_observe_filtered_pipeline_rows_and_empty_upstream() {
    let source = Inputs::new(
        10,
        &[
            ("A", "SELECT Id,Score FROM Data WHERE Id<=8"),
            ("B", "SELECT Id,Score FROM A WHERE Id Mod 2=0"),
            ("Empty", "SELECT Id,Score FROM A WHERE False"),
        ],
    );
    let (summary, rows) = run(
        "SELECT Count(*),Count(Score),Sum(Score) FROM B",
        &source,
        &[],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        rows,
        [vec![Value::Int64(4), Value::Int64(4), Value::Int64(60)]]
    );
    assert_eq!(summary.scanned_rows, 10);
    assert_eq!(summary.emitted_rows, 1);
    let (_, rows) = run(
        "SELECT CLng(Id Mod 3),Count(*),Sum(Score) FROM B GROUP BY Id Mod 3",
        &source,
        &[],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        rows,
        [
            vec![Value::Int32(2), Value::Int64(2), Value::Int64(30)],
            vec![Value::Int32(1), Value::Int64(1), Value::Int64(12)],
            vec![Value::Int32(0), Value::Int64(1), Value::Int64(18)]
        ]
    );
    assert_eq!(
        run(
            "SELECT Count(*),Sum(Score) FROM Empty",
            &source,
            &[],
            Default::default()
        )
        .unwrap()
        .1,
        [vec![Value::Int64(0), Value::Null]]
    );
    assert!(
        run(
            "SELECT Id,Count(*) FROM Empty GROUP BY Id",
            &source,
            &[],
            Default::default()
        )
        .unwrap()
        .1
        .is_empty()
    );
}

#[test]
fn each_top_counts_its_own_rows_before_downstream_filters_and_preserves_physical_eof() {
    for (sql, expected) in [
        (
            "SELECT Id FROM (SELECT TOP 2 Id FROM Data) Q WHERE Id>=3",
            vec![],
        ),
        (
            "SELECT TOP 1 Id FROM (SELECT TOP 3 Id FROM Data) Q WHERE Id>=2",
            vec![vec![Value::Int32(2)]],
        ),
        (
            "SELECT Id FROM (SELECT TOP 2 Id FROM Data) Q WHERE 1/(Id-3)=0",
            vec![],
        ),
    ] {
        let source = Inputs::new(5, &[]);
        let (summary, rows) = run(sql, &source, &[], Default::default()).unwrap();
        assert_eq!(rows, expected);
        assert_eq!(summary.scanned_rows, 5);
        assert_eq!(summary.emitted_rows, rows.len() as u64);
        assert_eq!(source.visits.get(), 5);
        assert!(source.eof.get());
    }
}
