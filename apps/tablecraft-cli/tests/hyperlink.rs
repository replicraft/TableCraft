mod support;

use support::run;

#[test]
fn hyperlink_query_selectors_preserve_components_without_following_addresses() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT HyperlinkPart('name#https://example.invalid#anchor#tip',0) AS D,HyperlinkPart('name#https://example.invalid#anchor#tip',5) AS F,HyperlinkPart(Null,2) AS N",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(
            rows,
            [
                vec!["D", "F", "N"],
                vec!["\"name\"", "\"https://example.invalid#anchor\"", "NULL"]
            ]
        );

        for expression in [
            "HyperlinkPart('x')",
            "HyperlinkPart('x',)",
            "HyperlinkPart(*)",
            "HyperlinkPart('x',6)",
            "HyperlinkPart('x',Null)",
            "HyperlinkPart(Null,1/0)",
            "HyperlinkPart('x',0),1/0",
            "VBA.HyperlinkPart('x',2)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}
