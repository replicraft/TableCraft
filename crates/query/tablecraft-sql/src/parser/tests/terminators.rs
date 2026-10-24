use super::*;

#[test]
fn statement_source_uses_the_final_lexical_terminator_and_preserves_original_sql() {
    for (source, expected) in [
        ("SELECT 1;", "SELECT 1"),
        ("SELECT ';' AS [;]; -- ; ignored", "SELECT ';' AS [;]"),
        ("SELECT \";\"; /* ; */", "SELECT \";\""),
        ("SELECT ';' -- ;\n; -- end", "SELECT ';' -- ;\n"),
        ("SELECT 'é🦀'\t;\r\n-- ;", "SELECT 'é🦀'\t"),
        ("SELECT\r\n\t'é🦀'; /* end */", "SELECT\r\n\t'é🦀'"),
        (
            "PARAMETERS [Prompt;?] LONG; SELECT [Prompt;?]; -- end",
            "PARAMETERS [Prompt;?] LONG; SELECT [Prompt;?]",
        ),
        ("SELECT #2026-10-24#;", "SELECT #2026-10-24#"),
        ("SELECT ';' -- ;", "SELECT ';' -- ;"),
        (
            "PARAMETERS P LONG; SELECT P /* ; */",
            "PARAMETERS P LONG; SELECT P /* ; */",
        ),
    ] {
        let parsed = parse_query(source, ParseLimits::default()).unwrap();
        assert_eq!(parsed.source(), source);
        assert_eq!(parsed.statement_source(), expected, "{source}");
        assert_eq!(
            parse_query(expected, ParseLimits::default())
                .unwrap()
                .query(),
            parsed.query()
        );
    }
}

#[test]
fn statement_source_does_not_repair_multiple_or_malformed_statements() {
    for source in [
        "SELECT 1;;",
        "SELECT 1; SELECT 2",
        "SELECT 1; /* ; */ SELECT 2;",
        "SELECT (1;",
        "SELECT 1);",
        "SELECT 'unterminated;",
        "SELECT 1; /* unterminated",
    ] {
        assert!(
            parse_query(source, ParseLimits::default()).is_err(),
            "{source}"
        );
    }
}
