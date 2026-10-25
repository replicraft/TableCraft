use super::*;
use std::process::Output;

fn query(root: &Database, target: &Database, sql: &str, options: &[&str]) -> Output {
    let mut arguments = vec![
        "query",
        "run",
        sql,
        "--database",
        root.path(),
        "--external",
        "remote",
        target.path(),
    ];
    arguments.extend_from_slice(options);
    run(&arguments)
}

#[test]
fn authorized_external_pipeline_leaves_keep_root_and_target_namespaces_distinct() {
    for profile in [1, 2, 5, 6] {
        let root = Database::new(image(profile, 10));
        let target = Database::new(image(profile, 100));
        for (sql, expected) in [
            (
                "SELECT Q.Id,Q.Score FROM (SELECT Id,Score FROM Items IN 'remote') Q WHERE Q.Id>=2",
                b"Id\tScore\n2\t200\n3\t300\n".as_slice(),
            ),
            (
                "SELECT Q.Id,Q.Score FROM (SELECT Id,Score FROM Items) Q WHERE Q.Id>=2",
                b"Id\tScore\n2\t20\n3\t30\n".as_slice(),
            ),
        ] {
            let buffered = query(&root, &target, sql, &[]);
            assert!(
                buffered.status.success(),
                "{}",
                String::from_utf8_lossy(&buffered.stderr)
            );
            assert_eq!(buffered.stdout, expected);
            for batch in ["1", "2"] {
                let result = query(&root, &target, sql, &["--stream", "--batch-rows", batch]);
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(result.stdout, expected);
            }
        }
    }
}

#[test]
fn external_pipeline_failures_keep_completed_prefixes_and_shared_native_limits() {
    let root = Database::new(image(6, 10));
    let target = Database::new(image(6, 100));
    let sql = "SELECT Q.Id FROM (SELECT Id,10\\(Id-2) AS Bad FROM Items IN 'remote') Q";
    for (batch, expected) in [("1", b"Id\n1\n".as_slice()), ("2", b"".as_slice())] {
        let result = query(&root, &target, sql, &["--stream", "--batch-rows", batch]);
        assert_eq!(result.status.code(), Some(2));
        assert_eq!(result.stdout, expected);
    }
    let exhausted = query(
        &root,
        &target,
        "SELECT Id FROM Items IN 'remote'",
        &["--stream", "--native-bytes", "1"],
    );
    assert_eq!(exhausted.status.code(), Some(2));
    assert!(exhausted.stdout.is_empty());
}
