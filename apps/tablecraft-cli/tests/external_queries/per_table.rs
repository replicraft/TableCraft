//! Explicit host mappings retain their identity at each qualified table leaf.
use super::*;

#[test]
fn local_and_two_external_tables_keep_distinct_values_across_profiles() {
    for profile in [1, 2, 5, 6] {
        let root = Database::new(image(profile, 10));
        let first = Database::new(image(profile, 100));
        let second = Database::new(image(profile, 1000));
        let output = run(&[
            "query",
            "run",
            "SELECT l.Id,l.Score+a.Score+b.Score AS Total FROM Items AS l INNER JOIN [alpha].[Items] AS a ON l.Id=a.Id INNER JOIN [beta].[Items] AS b ON a.Id=b.Id ORDER BY l.Id",
            "--database",
            root.path(),
            "--external",
            "alpha",
            first.path(),
            "--external",
            "beta",
            second.path(),
        ]);
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"Id\tTotal\n1\t1110\n2\t2220\n3\t3330\n");
    }
}

#[test]
fn per_table_tokens_preserve_punctuation_and_exact_mapping_case() {
    let root = Database::new(image(6, 10));
    let target = Database::new(image(6, 100));
    for (token, sql) in [
        (
            "opaque ' é [x]; -- token",
            "SELECT Sum(Score) AS Total FROM [opaque ' é [x]]; -- token].[Items]",
        ),
        (
            r"C:\temp\Database1.accdb",
            r"SELECT Sum(Score) AS Total FROM [C:\temp\Database1.accdb].[Items]",
        ),
    ] {
        let output = run(&[
            "query",
            "run",
            sql,
            "--database",
            root.path(),
            "--external",
            token,
            target.path(),
        ]);
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"Total\n600\n");
    }

    for mappings in [vec![], vec!["--external", "REMOTE", target.path()]] {
        let mut arguments = vec![
            "query",
            "run",
            "SELECT Id FROM [remote].[Items]",
            "--database",
            root.path(),
        ];
        arguments.extend(mappings);
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn explicit_prefix_overrides_global_in_without_changing_nested_lexical_owner() {
    let root = Database::new(image(6, 10));
    let first = Database::new(image(6, 100));
    let second = Database::new(image(6, 1000));
    let output = run(&[
        "query",
        "run",
        "SELECT a.Id,a.Score+b.Score AS Total,(SELECT Sum(Score) FROM Items) AS RootTotal FROM Items AS a,[other].[Items] AS b IN 'default' WHERE a.Id=b.Id ORDER BY a.Id",
        "--database",
        root.path(),
        "--external",
        "default",
        first.path(),
        "--external",
        "other",
        second.path(),
    ]);
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        b"Id\tTotal\tRootTotal\n1\t1100\t60\n2\t2200\t60\n3\t3300\t60\n"
    );
}

#[test]
fn qualified_target_retains_password_handling() {
    let root = Database::new(image(6, 10));
    let target = Database::new(agile_fixture::encrypted_image(
        &image(6, 100),
        6,
        "public qualified password",
    ));
    let arguments = [
        "query",
        "run",
        "SELECT Sum(Score) AS Total FROM [remote].[Items]",
        "--database",
        root.path(),
        "--external",
        "remote",
        target.path(),
    ];
    let mut correct = arguments.to_vec();
    correct.extend(["-p", "public qualified password"]);
    let output = run(&correct);
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"Total\n600\n");

    let mut wrong = arguments.to_vec();
    wrong.extend(["-p", "wrong password"]);
    for arguments in [arguments.to_vec(), wrong] {
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .to_ascii_lowercase()
                .contains("password")
        );
    }
}

#[test]
fn per_table_index_failures_do_not_fall_back_to_sequential_reads() {
    let root = Database::new(image(6, 10));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let target = Database::new(fs::read(path).unwrap());
    for (index, success) in [("ORDR_NBR", true), ("MissingIndex", false)] {
        let output = run(&[
            "query",
            "run",
            "SELECT Count(*) AS N FROM [remote].[ROULETTE_TB]",
            "--database",
            root.path(),
            "--external",
            "remote",
            target.path(),
            "--external-index",
            "remote",
            "ROULETTE_TB",
            index,
        ]);
        assert_eq!(output.status.success(), success, "{:?}", output);
        if success {
            assert_eq!(output.stdout, b"N\n38\n");
        } else {
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn qualified_reads_preserve_buffered_late_failure_and_shared_allowances() {
    let root = Database::new(image(6, 10));
    let target = Database::new(image(6, 100));
    for (sql, limits) in [
        (
            "SELECT Id,1/(3-Id) AS FailsLate FROM [remote].[Items]",
            vec![],
        ),
        (
            "SELECT Id FROM [remote].[Items]",
            vec!["--execution-rows", "1"],
        ),
        (
            "SELECT Id FROM [remote].[Items]",
            vec!["--native-steps", "1"],
        ),
        (
            "SELECT Id FROM [remote].[Items]",
            vec!["--output-bytes", "1"],
        ),
        ("SELECT Id FROM [remote].[Items]", vec!["--stream"]),
    ] {
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
        arguments.extend(limits);
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn qualified_update_cannot_edit_a_same_named_local_table_or_create_an_output() {
    let root = Database::new(image(1, 10));
    let destination = root.path.with_extension("qualified-output.mdb");
    assert!(!destination.exists());
    let output = run(&[
        "query",
        "update",
        "UPDATE [remote].[Items] SET Score=Score+7 WHERE Id=2",
        "--database",
        root.path(),
        "--output",
        destination.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!destination.exists());
}
