mod support;
use support::run;
fn table(bytes: &[u8]) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(bytes)
        .records()
        .map(|record| record.unwrap().iter().map(str::to_owned).collect())
        .collect()
}
#[test]
fn explicit_code_pages_work_in_scalar_and_query_commands_without_a_default() {
    for mode in ["eval", "run"] {
        for (page, expected) in [("1252", "\"ÀÀ\""), ("1251", "\"АА\"")] {
            let result = run(&[
                "query",
                mode,
                "SELECT String(2,192) AS C,String(2,'é') AS T",
                "--character-code-page",
                page,
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                table(&result.stdout),
                [vec!["C", "T"], vec![expected, "\"éé\""]]
            );
        }
        for (sql, page) in [
            ("SELECT 1,String(1,128)", None),
            ("SELECT 1,String(1,170)", Some("1253")),
        ] {
            let mut args = vec!["query", mode, sql];
            if let Some(page) = page {
                args.extend(["--character-code-page", page]);
            }
            let result = run(&args);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
}
#[test]
fn unsupported_settings_fail_before_database_io_and_inspection_stays_inert() {
    let result = run(&[
        "query",
        "run",
        "SELECT String(1,128)",
        "--database",
        "missing.accdb",
        "--character-code-page",
        "932",
    ]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(error.contains("supported single-byte code page"));
    assert!(!error.contains("No such file"));
    for command in [
        vec![
            "query",
            "parse",
            "SELECT String(1,128)",
            "--character-code-page",
            "1252",
        ],
        vec!["inspect", "missing.accdb", "--character-code-page", "1252"],
    ] {
        let result = run(&command);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("unexpected argument"));
    }
}
