use super::*;
use std::{ops::Range, process::Output};

const FORMAT_OFFSET: usize = 20;
const IDENTIFIER_RANGE: Range<usize> = 4..20;
const JET4_PROFILE: u8 = 1;
const JET_MAGIC: &[u8; 16] = b"Standard Jet DB\0";
const ACE_MAGIC: &[u8; 16] = b"Standard ACE DB\0";

fn databases(profile: u8) -> (Database, Database) {
    let mut root = linked_fixture::database(
        6,
        Some("/stored/path/must/not/be/opened.accdb"),
        Some("items"),
        None,
    );
    let mut target = target_fixture::database();
    let magic = if profile == JET4_PROFILE {
        JET_MAGIC
    } else {
        ACE_MAGIC
    };
    for bytes in [&mut root, &mut target] {
        bytes[FORMAT_OFFSET] = profile;
        bytes[IDENTIFIER_RANGE].copy_from_slice(magic);
    }
    (Database::new(root), Database::new(target))
}

fn query(root: &Database, target: &Database, sql: &str, extra: &[&str]) -> Output {
    let mut arguments = vec![
        "query",
        "run",
        sql,
        "--database",
        root.path(),
        "--link",
        "Linked",
        target.path(),
    ];
    arguments.extend_from_slice(extra);
    run(&arguments)
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}

#[test]
fn mapped_link_streams_match_buffered_rows_for_priority_profiles_and_batches() {
    for profile in [1, 2, 5, 6] {
        let (root, target) = databases(profile);
        let sql = "SELECT Id,Score FROM Linked";
        let expected = b"Id\tScore\n1\t10\n2\t20\n3\t30\n";
        assert_eq!(success(query(&root, &target, sql, &[])), expected);
        for batch in ["1", "2"] {
            assert_eq!(
                success(query(
                    &root,
                    &target,
                    sql,
                    &["--stream", "--batch-rows", batch]
                )),
                expected
            );
        }
    }
}

#[test]
fn mapped_alias_union_arms_share_the_existing_aggregate_evaluator() {
    let (root, target) = databases(6);
    let sql = concat!(
        "SELECT Count(*) AS N FROM Linked UNION ALL ",
        "SELECT Sum(Score) FROM Linked UNION ALL SELECT Min(Id) FROM Linked UNION ALL ",
        "SELECT Max(Score) FROM Linked UNION ALL SELECT Id FROM Linked WHERE Id=3",
    );
    let expected = b"N\n3\n60\n1\n30\n3\n";
    assert_eq!(success(query(&root, &target, sql, &[])), expected);
    assert_eq!(
        success(query(
            &root,
            &target,
            sql,
            &["--stream", "--batch-rows", "1"]
        )),
        expected
    );

    let grouped =
        "SELECT CLng(Id Mod 2) AS K,Count(*) AS N,Sum(Score) AS S FROM Linked GROUP BY Id Mod 2";
    let expected = b"K\tN\tS\n1\t2\t40\n0\t1\t20\n";
    assert_eq!(success(query(&root, &target, grouped, &[])), expected);
    for batch in ["1", "2"] {
        assert_eq!(
            success(query(
                &root,
                &target,
                grouped,
                &["--stream", "--batch-rows", batch]
            )),
            expected
        );
    }
}

#[test]
fn linked_eager_failures_retain_only_previously_completed_tsv_batches() {
    let (root, target) = databases(6);
    let sql = "SELECT Id,10\\(Id-2) AS V FROM Linked";
    for (extra, expected) in [
        (vec![], b"".as_slice()),
        (
            vec!["--stream", "--batch-rows", "1"],
            b"Id\tV\n1\t-10\n".as_slice(),
        ),
        (vec!["--stream", "--batch-rows", "2"], b"".as_slice()),
    ] {
        let output = query(&root, &target, sql, &extra);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn streaming_does_not_open_unmapped_or_chained_stored_paths() {
    let (root, _) = databases(6);
    let unmapped = run(&[
        "query",
        "run",
        "SELECT Id FROM Linked",
        "--database",
        root.path(),
        "--stream",
    ]);
    assert_eq!(unmapped.status.code(), Some(2));
    assert!(unmapped.stdout.is_empty());

    let front = Database::new(linked_fixture::database(
        6,
        Some("/ignored/front.accdb"),
        Some("Linked"),
        None,
    ));
    let output = query(&front, &root, "SELECT Id FROM Linked", &["--stream"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
