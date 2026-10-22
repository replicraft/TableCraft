mod support;

use support::run;

#[test]
fn text_masks_sections_casing_and_unicode_execute_in_both_commands() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Format('Ab','@@@@') AS R,Format('Ab','!@@@@') AS L,Format(Null,'@@;\"missing\"') AS N,Format('straße','>\"label:\"@@@@@@') AS C,Format('🦀','@@') AS U",
            "--case-locale",
            "und",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            [
                "\"  Ab\"",
                "\"Ab  \"",
                "\"missing\"",
                "\"label:STRASSE\"",
                "\"🦀\""
            ]
        );
        for expression in [
            "Format('🦀','@-@')",
            "Format('x','><@@')",
            "Format(Null,'@@',1/0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
