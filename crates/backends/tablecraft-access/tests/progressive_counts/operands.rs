use super::*;

const VALUES: [Option<i128>; 5] = [Some(1), None, Some(0), None, Some(-3)];

fn decimal_database(profile: u8) -> Vec<u8> {
    decimal_fixture::database_for(profile, false, 28, 4, &VALUES, 0)
}

fn invalid_last_decimal(mut bytes: Vec<u8>) -> Vec<u8> {
    let directory = ORDINARY_DATA_PAGE * PAGE_BYTES
        + ROW_DIRECTORY_OFFSET
        + (VALUES.len() - 1) * size_of::<u16>();
    let row = u16::from_le_bytes(
        bytes[directory..directory + size_of::<u16>()]
            .try_into()
            .unwrap(),
    );
    bytes[ORDINARY_DATA_PAGE * PAGE_BYTES + usize::from(row) + DECIMAL_VALUE_OFFSET] = 0x7f;
    bytes
}

#[test]
fn nullable_native_operands_keep_distinct_counters_without_snapshot_retention() {
    for profile in [1, 2, 5, 6] {
        let bytes = decimal_database(profile);
        let original = bytes.clone();
        let native = NativeSource::new_with_snapshot_limits(
            Cursor::new(bytes.as_slice()),
            SCOPE,
            budget(),
            &|| false,
            TableSnapshotLimits::new(1, 1).unwrap(),
        )
        .unwrap();
        let sql = concat!(
            "SELECT Count(*) AS AllRows,Count(ID) AS IdRows,Count(Score) AS ScoreRows,",
            "Count(IIf(ID<=2,Score,Null)) AS ConditionalRows,",
            "Count(0) AS LiteralRows,Count(Null) AS NullRows FROM Items",
        );
        let query = plan(sql, &native);
        let expected = vec![vec![
            Value::Int64(5),
            Value::Int64(5),
            Value::Int64(3),
            Value::Int64(1),
            Value::Int64(5),
            Value::Int64(0),
        ]];
        assert_eq!(streamed(&query, &native).unwrap(), (expected.clone(), 5, 1));
        assert_eq!(
            query.execute(&native, &[], Default::default()).err(),
            Some(QueryError::StructureLimit),
        );
        assert_eq!(streamed(&query, &native).unwrap().0, expected);
        assert_eq!(
            query.execute(&native, &[], Default::default()).err(),
            Some(QueryError::StructureLimit),
        );

        let filtered = plan("SELECT Count(Score) FROM Items WHERE ID<=2", &native);
        assert_eq!(
            streamed(&filtered, &native).unwrap(),
            (vec![vec![Value::Int64(1)]], 5, 1)
        );
        let empty = plan(
            "SELECT Count(Score),Count(ID) FROM Items WHERE False",
            &native,
        );
        assert_eq!(
            streamed(&empty, &native).unwrap(),
            (vec![vec![Value::Int64(0), Value::Int64(0)]], 5, 1),
        );
        let union = plan(
            concat!(
                "SELECT Count(Score) AS N FROM Items UNION ALL ",
                "SELECT Count(ID) FROM Items UNION ALL ",
                "SELECT Count(Null) FROM Items",
            ),
            &native,
        );
        assert_eq!(
            streamed(&union, &native).unwrap(),
            (
                vec![
                    vec![Value::Int64(3)],
                    vec![Value::Int64(5)],
                    vec![Value::Int64(0)]
                ],
                15,
                3,
            ),
        );
        drop(native);
        assert_eq!(bytes, original);
    }
}

#[test]
fn count_operand_selection_prunes_unused_decimal_payloads_but_checks_selected_values() {
    for profile in [1, 2, 5, 6] {
        let bytes = invalid_last_decimal(decimal_database(profile));
        let original = bytes.clone();
        let native = NativeSource::new_with_snapshot_limits(
            Cursor::new(bytes.as_slice()),
            SCOPE,
            budget(),
            &|| false,
            TableSnapshotLimits::new(1, 1).unwrap(),
        )
        .unwrap();
        for sql in [
            "SELECT Count(*) FROM Items",
            "SELECT Count(ID) FROM Items",
            "SELECT Count(IIf(True,ID,Null)) FROM Items",
        ] {
            assert_eq!(
                streamed(&plan(sql, &native), &native).unwrap().0,
                [vec![Value::Int64(5)]]
            );
        }
        for sql in [
            "SELECT Count(Score) FROM Items",
            "SELECT Count(*),Count(Score) FROM Items",
            "SELECT Count(IIf(True,ID,Score)) FROM Items",
        ] {
            assert!(matches!(
                refused_without_count(&plan(sql, &native), &native),
                StreamError::Query(QueryError::Unsupported),
            ));
        }
        assert_eq!(
            streamed(&plan("SELECT Count(ID) FROM Items", &native), &native)
                .unwrap()
                .0,
            [vec![Value::Int64(5)]],
        );
        drop(native);
        assert_eq!(bytes, original);
    }
}

#[test]
fn eager_count_operand_errors_discard_the_aggregate_and_do_not_start_later_arms() {
    let (bytes, other_data) = with_other(decimal_database(6));
    let watched = Watched::new(bytes);
    let pages = watched.pages.clone();
    let native = source(watched, &|| false);
    let query = plan(
        concat!(
            "SELECT Count(IIf(True,ID,10\\(ID-5))) FROM Items UNION ALL ",
            "SELECT Count(ID) FROM Other",
        ),
        &native,
    );
    assert!(matches!(
        refused_without_count(&query, &native),
        StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero)),
    ));
    assert!(!pages.borrow().contains(&other_data));
    assert_eq!(
        streamed(&plan("SELECT Count(Score) FROM Items", &native), &native)
            .unwrap()
            .0,
        [vec![Value::Int64(3)]],
    );
}

#[test]
fn union_count_operands_share_native_budgets_and_keep_only_completed_aggregates() {
    let (bytes, other_data) = with_other(decimal_database(5));
    let watched = Watched::new(bytes);
    let pages = watched.pages.clone();
    let pool = RefCell::new(budget());
    let native = NativeSource::new_with_shared_budget_and_snapshot_limits(
        watched,
        SCOPE,
        &pool,
        &|| false,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap();
    let query = plan(
        "SELECT Count(Score) FROM Items UNION ALL SELECT Count(ID) FROM Other",
        &native,
    );
    let mut rows = Vec::new();
    let result = query.execute_progressive(&native, &[], Default::default(), &mut |row| {
        rows.push(row.to_vec());
        let mut remaining = pool.try_borrow_mut().unwrap();
        let allowance = remaining.remaining(Resource::Rows);
        remaining.charge(Resource::Rows, allowance).unwrap();
        Ok::<(), ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(rows, [vec![Value::Int64(3)]]);
    assert!(!pages.borrow().contains(&other_data));
}

#[test]
fn later_count_operand_failure_never_publishes_a_partial_second_counter() {
    let native = source(
        Watched::new(invalid_last_decimal(decimal_database(6))),
        &|| false,
    );
    let query = plan(
        "SELECT Count(ID) FROM Items UNION ALL SELECT Count(Score) FROM Items",
        &native,
    );
    let mut rows = Vec::new();
    let result = query.execute_progressive(&native, &[], Default::default(), &mut |row| {
        rows.push(row.to_vec());
        Ok::<(), ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Unsupported))
    ));
    assert_eq!(rows, [vec![Value::Int64(5)]]);
}

#[test]
fn operand_counts_honor_native_cancellation_before_aggregate_publication() {
    let watched = Watched::new(decimal_database(6));
    let stopped = watched.stopped.clone();
    watched.cancel_page.set(Some(ORDINARY_DATA_PAGE));
    let cancel = || stopped.get();
    let native = source(watched, &cancel);
    let query = plan("SELECT Count(Score),Count(ID) FROM Items", &native);
    assert!(matches!(
        refused_without_count(&query, &native),
        StreamError::Query(QueryError::Cancelled),
    ));
}
