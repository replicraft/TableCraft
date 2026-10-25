use std::process::Command;

#[test]
fn both_query_modes_preserve_japanese_separators_and_explicit_locale_precedence() {
    for mode in ["eval", "run"] {
        for (expression, options, expected) in [
            (
                "StrConv('hello　WORLD',3,1041)",
                &[][..],
                "\"Hello　World\"",
            ),
            (
                "StrConv('hello　WORLD',3,1033)",
                &[][..],
                "\"Hello　world\"",
            ),
            (
                "StrConv('ｈＥＬＬＯ　ｗＯＲＬＤ',11,1041)",
                &[][..],
                "\"Hello World\"",
            ),
            (
                "StrConv('hello　WORLD',3)",
                &["--case-locale", "ja-JP"][..],
                "\"Hello　World\"",
            ),
            (
                "StrConv('hello　WORLD',3,1033)",
                &["--case-locale", "ja-JP"][..],
                "\"Hello　world\"",
            ),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args([
                    "query",
                    mode,
                    &format!("SELECT {expression} AS V"),
                    "--format",
                    "tsv",
                ])
                .args(options)
                .output()
                .unwrap();
            assert!(output.status.success(), "{:?}", output.stderr);
            assert!(output.stderr.is_empty());
            let mut reader = csv::ReaderBuilder::new()
                .delimiter(b'\t')
                .from_reader(output.stdout.as_slice());
            let row = reader.records().next().unwrap().unwrap();
            assert_eq!(&row[0], expected);
        }
    }
}
