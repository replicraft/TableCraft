use super::*;
use proptest::prelude::*;

struct Ordinal;
impl tablecraft_expressions::comparison::Collation for Ordinal {
    fn compare(
        &self,
        left: &str,
        right: &str,
    ) -> Result<std::cmp::Ordering, tablecraft_expressions::EvalError> {
        Ok(left.encode_utf16().cmp(right.encode_utf16()))
    }
}

fn run_text(sql: &str, source: &Attachments) -> Result<QueryResult, QueryError> {
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(query.query(), source)?.execute(
        source,
        &[],
        ExecutionOptions {
            collation: Some(&Ordinal),
            ..Default::default()
        },
    )
}
fn scored() -> Attachments {
    let mut source = Attachments::new();
    // Case variants must join the same collection, even in interleaved metadata.
    source.members.push(
        CollectionMember::new(
            "fIlEs".into(),
            Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
        )
        .unwrap(),
    );
    for (lists, scores) in source.lists.iter_mut().zip([
        vec![Value::Int32(1), Value::Int32(0)],
        vec![],
        vec![Value::Int32(-1)],
    ]) {
        lists.push(scores);
    }
    source
}

#[test]
fn hidden_members_filter_the_displayed_child_across_aliases_grouping_and_pruning() {
    let source = scored();
    let sql = "SELECT T.Id,T.Files.FileName AS N FROM Items AS T WHERE t.FILES.FileType='txt' AND T.files.Score=1";
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1), text("a")]]
    );
    for sql in [
        "SELECT Files.FileName FROM Items WHERE Files.FileType='png'",
        "SELECT D.N FROM (SELECT Files.FileName AS N FROM Items WHERE Files.FileType='png') AS D",
    ] {
        assert_eq!(run_text(sql, &source).unwrap().rows(), [vec![text("b")]]);
    }
    assert_eq!(
        run_text(
            "SELECT Count(Files.FileName) FROM Items WHERE Files.FileType='txt'",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int64(1)]]
    );
    assert_eq!(run_text("SELECT Files.FileName,Count(*) FROM Items WHERE Files.FileType='txt' GROUP BY Files.FileName", &source).unwrap().rows(), [vec![text("a"), Value::Int64(1)]]);
    assert_eq!(
        run_text(
            "SELECT Files.FileName,Other.Label FROM Items WHERE Files.FileType='png'",
            &source
        )
        .unwrap()
        .rows(),
        [vec![text("b"), text("x")], vec![text("b"), text("y")]]
    );
    assert_eq!(
        run(
            "SELECT Id,Files.FileType FROM Items WHERE Files.FileName IS NOT NULL",
            &source
        )
        .unwrap()
        .rows(),
        [
            vec![Value::Int32(1), text("txt")],
            vec![Value::Int32(1), text("png")]
        ]
    );
    assert_eq!(
        run_text(
            "SELECT Id,Files.FileName FROM Items ORDER BY Files.FileType,Id",
            &source
        )
        .unwrap()
        .rows(),
        [
            vec![Value::Int32(2), Value::Null],
            vec![Value::Int32(3), Value::Null],
            vec![Value::Int32(1), text("b")],
            vec![Value::Int32(1), text("a")]
        ]
    );
    assert_eq!(run_text("SELECT Files.FileName,Count(*) FROM Items GROUP BY Files.FileName HAVING Count(Files.FileType)>0 ORDER BY Files.FileName", &source).unwrap().rows(), [vec![Value::Null, Value::Int64(2)], vec![text("a"), Value::Int64(1)], vec![text("b"), Value::Int64(1)]]);
}

#[test]
fn qualified_collections_in_joins_and_correlated_reads_keep_distinct_identities() {
    let source = Attachments::new();
    assert_eq!(run_text("SELECT A.Id,A.Files.FileName,B.Files.FileName FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE A.Files.FileType='png' AND B.Files.FileType='txt'", &source).unwrap().rows(), [vec![Value::Int32(1), text("b"), text("a")]]);
    assert_eq!(run_text("SELECT A.Files.FileName FROM Items AS A WHERE EXISTS(SELECT B.Id FROM Items AS B WHERE B.Id=A.Id AND A.Files.FileType='png')", &source).unwrap().rows(), [vec![text("b")]]);
    let source = Attachments::new();
    for sql in [
        "SELECT A.Files.FileName FROM Items AS A,Items AS B WHERE B.Files.FileType IS NULL",
        "SELECT Files.FileName FROM Items WHERE Other.Label IS NULL",
        "SELECT Files.FileName FROM Items WHERE Files.FileType IS NULL OR Other.Label IS NULL",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn hidden_members_preserve_later_errors_alignment_and_execution_limits() {
    let source = scored();
    for sql in [
        "SELECT TOP 1 Files.FileName FROM Items WHERE 1 \\ Files.Score>0",
        "SELECT Files.FileName FROM Items WHERE False AND 1 \\ Files.Score>0",
        "SELECT Files.FileName FROM Items WHERE Id=1 OR 1 \\ Files.Score>0",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(
                tablecraft_expressions::EvalError::DivisionByZero
            )),
            "{sql}"
        );
    }
    let query = parse_query(
        "SELECT Files.FileName FROM Items WHERE Files.FileType IS NOT NULL",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for options in [
        ExecutionOptions {
            cancelled: Some(&|| true),
            ..Default::default()
        },
        ExecutionOptions {
            limits: tablecraft_query::QueryLimits {
                rows: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    ] {
        assert!(plan.execute(&source, &[], options).is_err());
    }
    let mut source = scored();
    source.ignore_errors = true;
    source.lists[0][3].pop();
    assert_eq!(
        run(
            "SELECT Files.FileName FROM Items WHERE Files.Score>0",
            &source
        )
        .err(),
        Some(QueryError::Source)
    );
    source.lists[0][3].push(text("wrong carrier"));
    assert!(matches!(
        run(
            "SELECT Files.FileName FROM Items WHERE Files.Score>0",
            &source
        ),
        Err(QueryError::Schema(_))
    ));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
    #[test]
    fn hidden_numeric_members_match_an_independent_zipped_child_model(values in prop::collection::vec(prop::collection::vec(prop::option::of(-2_i32..3), 0..5), 0..8), threshold in -2_i32..3) {
        let mut source = scored();
        source.lists = values.iter().map(|scores| vec![
            scores.iter().enumerate().map(|(i,_)| text(&format!("child-{i}"))).collect(), vec![],
            scores.iter().map(|_| text("txt")).collect(), scores.iter().map(|v| v.map_or(Value::Null, Value::Int32)).collect(),
        ]).collect();
        let expected = values.iter().enumerate().flat_map(|(parent,scores)| scores.iter().enumerate().filter_map(move |(child, score)| score.filter(|score| *score>threshold).map(|_| vec![Value::Int32(parent as i32+1), text(&format!("child-{child}"))]))).collect::<Vec<_>>();
        let actual = run(&format!("SELECT Id,Files.FileName FROM Items WHERE Files.Score>{threshold}"), &source).unwrap();
        prop_assert_eq!(actual.rows(), expected);
    }
}

#[test]
fn a_full_width_metadata_schema_still_binds_hidden_sibling_predicates() {
    let mut source = Attachments::new();
    // One scalar + two unavailable names + three initial members + 250 added
    // members reach the independently specified 256-slot source boundary.
    for index in 0..250 {
        source.members.push(
            CollectionMember::new(
                "Files".into(),
                Column::new(format!("N{index}"), ScalarType::Int32, true).unwrap(),
            )
            .unwrap(),
        );
        for lists in &mut source.lists {
            lists.push(vec![Value::Int32(1); lists[0].len()]);
        }
    }
    assert_eq!(
        run(
            "SELECT Files.FileName FROM Items WHERE Files.N249>0",
            &source
        )
        .unwrap()
        .rows(),
        [vec![text("a")], vec![text("b")], vec![Value::Null]]
    );
}

#[test]
fn parameters_rebind_and_all_execution_cancellation_checkpoints_remain_effective() {
    let source = Attachments::new();
    let query = parse_query(
        "PARAMETERS Needle TEXT(10); SELECT Files.FileName FROM Items WHERE Files.FileType=Needle",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (needle, name) in [("txt", "a"), ("png", "b")] {
        assert_eq!(
            plan.execute(
                &source,
                &[("Needle", text(needle))],
                ExecutionOptions {
                    collation: Some(&Ordinal),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![text(name)]]
        );
    }
    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let execute = || {
        plan.execute(
            &source,
            &[("Needle", text("txt"))],
            ExecutionOptions {
                collation: Some(&Ordinal),
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    execute().unwrap();
    let total = calls.get();
    assert!(total > 10);
    for checkpoint in 1..=total {
        calls.set(0);
        stop.set(checkpoint);
        assert_eq!(execute().err(), Some(QueryError::Cancelled));
    }
}
