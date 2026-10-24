mod support;

use support::run;

fn query(sql: &str, options: &[&str]) -> std::process::Output {
    let mut arguments = vec!["query", "run", sql];
    arguments.extend_from_slice(options);
    run(&arguments)
}

#[test]
fn explicit_icu_text_mode_compares_searches_and_replaces_original_spans() {
    let output = query(
        "SELECT StrComp('A','a',1) AS C,StrComp('é','é',1) AS N,InStr(1,'😀AéZ','É',1) AS F,InStrRev('aAa','a',-1,1) AS R,Replace('XéZ','É','Q',1,-1,1) AS P,Replace('ﬁlms','fi','X',1,-1,1) AS E",
        &["--text-locale", "en-US"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let row = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(output.stdout.as_slice())
        .records()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(
        row.iter().collect::<Vec<_>>(),
        ["0", "0", "4", "3", "\"XQZ\"", "\"Xlms\""]
    );
}

#[test]
fn explicit_locale_tailoring_and_lcid_registration_are_separate_choices() {
    for (locale, expected) in [
        ("de-DE", b"C\n-1\n".as_slice()),
        ("sv-SE", b"C\n1\n".as_slice()),
    ] {
        let output = query("SELECT StrComp('ö','z',1) AS C", &["--text-locale", locale]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected);
    }
    let output = query(
        "SELECT InStr(1,'xI','ı',1055) AS N",
        &["--text-locale", "tr-TR", "--text-lcid", "1055"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"N\n2\n");
    for options in [
        vec!["--text-locale", "tr-TR"],
        vec!["--text-locale", "tr-TR", "--text-lcid", "1033"],
        vec!["--text-locale", "tr-TR", "--text-lcid", "66591"],
    ] {
        let output = query("SELECT 1 AS Prefix,InStr(1,'xI','ı',1055)", &options);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn ordinary_sql_binary_and_database_comparison_do_not_become_icu_text() {
    let output = query(
        "SELECT 'a'='A' AS E,StrComp('a','A',1) AS T,StrComp('a','A',2) AS D",
        &["--text-locale", "en-US", "--collation", "ordinal"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"E\tT\tD\nFALSE\t0\t1\n");
    for sql in [
        "SELECT 'a'='A'",
        "SELECT StrComp('a','A')",
        "SELECT 'a' LIKE 'A'",
    ] {
        let output = query(sql, &["--text-locale", "en-US", "--like-mode", "ansi89"]);
        assert_eq!(output.status.code(), Some(2), "{sql}");
        assert!(output.stdout.is_empty());
    }
    let output = query(
        "SELECT StrComp(ChrW(55296),ChrW(55296),0) AS B",
        &["--text-locale", "en-US"],
    );
    assert!(output.status.success());
    assert_eq!(output.stdout, b"B\n0\n");
    for sql in [
        "SELECT StrComp(ChrW(55296),'a',1)",
        "SELECT InStr(1,ChrW(55296),'a',1)",
    ] {
        let output = query(sql, &["--text-locale", "en-US"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn native_field_resolver_and_automatic_indexes_keep_database_policy() {
    let output = query(
        "SELECT TAG_GRP_ID,StrComp(TAG_GRP_NME,'office objects',1) AS T FROM TAG_GRP_TB WHERE TAG_GRP_NME='Office Objects'",
        &[
            "--database",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/examples/Access_Example_VBA.accdb"
            ),
            "--collation",
            "database",
            "--auto-index",
            "--text-locale",
            "en-US",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"TAG_GRP_ID\tT\n9\t0\n");
}

#[test]
fn cumulative_provider_quotas_and_hard_input_bounds_fail_buffered() {
    let sql = "SELECT 1 AS Prefix,StrComp('a','A',1),StrComp('b','B',1)";
    let output = query(
        sql,
        &[
            "--text-locale",
            "en-US",
            "--comparison-steps",
            "7",
            "--execution-steps",
            "1000000",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let output = query(sql, &["--text-locale", "en-US", "--comparison-steps", "8"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = query(
        "SELECT InStr(1,'abc','B',1)",
        &["--text-locale", "en-US", "--comparison-bytes", "1"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    for sql in [
        "SELECT StrComp(String(65537,'a'),'a',1)",
        "SELECT InStr(1,String(4097,'a'),'z',1)",
    ] {
        let output = query(
            sql,
            &[
                "--text-locale",
                "en-US",
                "--text-bytes",
                "65537",
                "--comparison-steps",
                "100000000",
            ],
        );
        assert_eq!(output.status.code(), Some(2), "{sql}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn invalid_provider_options_fail_before_database_io() {
    for options in [
        vec!["--text-locale", "unknown"],
        vec!["--text-locale", "en-US-u-co-search"],
        vec!["--text-lcid", "1033"],
        vec!["--text-locale", "en-US", "--text-lcid", "0"],
        vec!["--text-locale", "en-US", "--text-lcid", "1048576"],
    ] {
        let mut options = options;
        options.extend(["--database", "missing.accdb"]);
        let output = query("SELECT 1", &options);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("failed to open"));
    }
}
