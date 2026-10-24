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
fn replacement_streams_to_buffered_results_and_keeps_access_suffix_rules() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT Replace('banana','an','X') AS A,Replace('banana','an','X',3,1) AS B,Replace('😀x😀','😀','é',3) AS C,Replace('aaaa','aa','a',,,) AS D",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            table(&result.stdout),
            [
                vec!["A", "B", "C", "D"],
                vec!["\"bXXa\"", "\"nXa\"", "\"xé\"", "\"aa\""]
            ]
        );
        for expression in [
            "Replace(NULL,'a','x')",
            "Replace('','a','x',0)",
            "Replace(NULL,'a','x',1/0)",
            "Replace('😀x','x','y',2)",
            "Replace('a','a','x',1,-1,NULL)",
        ] {
            let result = run(&["query", command, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
#[test]
fn selected_ordinal_provider_handles_replacement_without_locale_fallback() {
    let result = run(&[
        "query",
        "run",
        "SELECT Replace('banana','an','X',,,2) AS A,Replace('banana','an','X',,1,-1) AS B",
        "--collation",
        "ordinal",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        table(&result.stdout),
        [vec!["A", "B"], vec!["\"bXXa\"", "\"bXana\""]]
    );
    let result = run(&[
        "query",
        "run",
        "SELECT 1,Replace('A','a','x',,,1)",
        "--collation",
        "ordinal",
    ]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
}

#[test]
fn retained_replacement_is_observable_and_final_unicode_stays_strict() {
    for command in ["eval", "run"] {
        let result = run(&[
            "query",
            command,
            "SELECT AscW(Replace('😀',ChrW(55357),'')) AS Code,LenB(Replace('😀','z','',2,0)) AS Bytes,Replace(ChrW(55296)&'a','a',ChrW(56320)) AS Repaired",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            table(&result.stdout),
            [
                vec!["Code", "Bytes", "Repaired"],
                vec!["-8704", "2", "\"𐀀\""]
            ]
        );
        for sql in [
            "SELECT Replace(ChrW(55296),'a','b')",
            "SELECT LenB(Replace(ChrW(55296),'a','b')),1/0",
            "SELECT LenB(Replace(ChrB(65),'a','b'))",
        ] {
            let result = run(&["query", command, sql]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}
