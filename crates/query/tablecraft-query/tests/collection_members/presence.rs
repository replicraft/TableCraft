use super::criteria::run_text;
use super::*;
use tablecraft_query::QueryLimits;

#[test]
fn bare_named_collection_null_tests_use_membership_not_nullable_metadata() {
    let source = Attachments::new();
    for (predicate, expected) in [
        ("Files IS NULL", vec![2]),
        ("[FILES] IS NULL", vec![2]),
        ("Files IS NOT NULL", vec![1, 3]),
        ("NOT (Files IS NULL)", vec![1, 3]),
        ("Other IS NULL", vec![3]),
        ("Other IS NOT NULL", vec![1, 2]),
        ("Files IS NULL OR Other IS NULL", vec![2, 3]),
        ("Files IS NOT NULL AND Other IS NOT NULL", vec![1]),
        ("Files IS NULL XOR Other IS NULL", vec![2, 3]),
    ] {
        let sql = format!("SELECT Id FROM Items WHERE {predicate} ORDER BY Id");
        let query = parse_query(&sql, ParseLimits::default()).unwrap();
        let scans = source.scans.get();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(source.scans.get(), scans);
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(
            result.rows(),
            expected
                .into_iter()
                .map(|id| vec![Value::Int32(id)])
                .collect::<Vec<_>>(),
            "{sql}"
        );
    }
}

#[test]
fn presence_keeps_displayed_members_parent_counts_and_qualified_join_locations() {
    let source = Attachments::new();
    for (sql, expected) in [
        (
            "SELECT T.Id,T.Files.FileName FROM Items AS T WHERE T.Files IS NOT NULL ORDER BY T.Id",
            vec![
                vec![Value::Int32(1), text("a")],
                vec![Value::Int32(1), text("b")],
                vec![Value::Int32(3), Value::Null],
            ],
        ),
        (
            "SELECT Id,Files.FileName FROM Items WHERE Files IS NULL",
            vec![vec![Value::Int32(2), Value::Null]],
        ),
        (
            "SELECT Count(*) FROM Items WHERE Files IS NOT NULL",
            vec![vec![Value::Int64(2)]],
        ),
        (
            "SELECT D.Id FROM (SELECT Id FROM Items WHERE Files IS NULL) AS D",
            vec![vec![Value::Int32(2)]],
        ),
        (
            "SELECT A.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE A.Files IS NULL AND B.Other IS NOT NULL",
            vec![vec![Value::Int32(2)]],
        ),
        (
            "SELECT A.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id=B.Id AND B.Id=1 WHERE B.Files IS NULL ORDER BY A.Id",
            vec![vec![Value::Int32(2)], vec![Value::Int32(3)]],
        ),
        (
            "TRANSFORM Count(*) SELECT Id FROM Items WHERE Files IS NULL GROUP BY Id PIVOT 1 IN (1)",
            vec![vec![Value::Int32(2), Value::Int64(1)]],
        ),
    ] {
        assert_eq!(run(sql, &source).unwrap().rows(), expected, "{sql}");
    }
}

#[test]
fn presence_remains_eager_and_validates_complete_views_after_true_or_false_prefixes() {
    let mut source = Attachments::new();
    for sql in [
        "SELECT Id FROM Items WHERE True OR Files IS NOT NULL",
        "SELECT TOP 1 Id FROM Items WHERE Files IS NOT NULL",
        "SELECT Id FROM Items WHERE False AND Files IS NULL",
    ] {
        source.lists[2][0] = vec![Value::Int32(42)];
        source.ignore_errors = true;
        assert!(run(sql, &source).is_err(), "{sql}");
    }

    let source = Attachments::new();
    assert!(run("SELECT Id FROM Items WHERE Files IS NULL OR 1/0", &source).is_err());
    for sql in [
        "SELECT Files IS NULL FROM Items",
        "SELECT Id FROM Items WHERE Files IS NULL AND Files.FileName",
        "SELECT Id FROM Items AS A,Items AS B WHERE Files IS NULL",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let scans = source.scans.get();
        assert!(prepare(query.query(), &source).is_err(), "{sql}");
        assert_eq!(source.scans.get(), scans);
    }
}

#[test]
fn presence_obeys_each_observed_cancellation_checkpoint() {
    let source = Attachments::new();
    let query = parse_query(
        "SELECT Id FROM Items WHERE Files IS NOT NULL",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let checks = Cell::new(0);
    let invoke = |stop| {
        checks.set(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() == stop
        };
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    assert!(invoke(usize::MAX).is_ok());
    let total = checks.get();
    for stop in 1..=total {
        assert_eq!(
            invoke(stop).err(),
            Some(QueryError::Cancelled),
            "checkpoint {stop}"
        );
    }
}

#[test]
fn presence_retains_cumulative_work_row_and_allocation_limits() {
    let source = Attachments::new();
    let query = parse_query(
        "SELECT Id FROM Items WHERE Files IS NOT NULL",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for limits in [
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn container_presence_never_converts_displayed_member_criteria_to_parent_wide_matches() {
    let source = Attachments::new();
    for predicate in [
        "Files IS NOT NULL AND Files.FileName='a'",
        "Files.FileName='a' AND Files IS NOT NULL",
        "NOT (Files IS NULL) AND Files.FileName='a'",
    ] {
        let sql = format!("SELECT Id,Files.FileName FROM Items WHERE {predicate}");
        assert_eq!(
            run_text(&sql, &source).unwrap().rows(),
            [vec![Value::Int32(1), text("a")]],
            "{sql}"
        );
    }
}

#[test]
fn alias_and_member_interpretations_of_presence_remain_ambiguous_before_scanning() {
    let mut source = Attachments::new();
    source.members.push(
        CollectionMember::new(
            "Files".into(),
            Column::new("Other".into(), ScalarType::Text, true).unwrap(),
        )
        .unwrap(),
    );
    let query = parse_query(
        "SELECT Id FROM Items AS Files WHERE Files.Other IS NULL",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source).err(),
        Some(QueryError::AmbiguousName)
    );
    assert_eq!(source.scans.get(), 0);
}
