mod support;
use support::run;
fn row(bytes: &[u8]) -> Vec<String> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(bytes)
        .records()
        .next()
        .unwrap()
        .unwrap()
        .iter()
        .map(str::to_owned)
        .collect()
}
#[test]
fn casing_is_explicit_in_eval_and_run_with_contextual_output() {
    for mode in ["eval", "run"] {
        for (locale, lower, upper) in [("und", "ii\u{307}iı", "IİII"), ("tr", "ıiiı", "IİİI")]
        {
            let result = run(&[
                "query",
                mode,
                "SELECT LCase('Iİiı'),UCase('Iİiı'),LCase(NULL),UCase('straße'),LCase('ΟΣ')",
                "--case-locale",
                locale,
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                row(&result.stdout),
                [
                    format!("\"{lower}\""),
                    format!("\"{upper}\""),
                    "NULL".into(),
                    "\"STRASSE\"".into(),
                    "\"ος\"".into()
                ]
            );
        }
        for expression in ["LCase('x')", "UCase('')"] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
        let result = run(&["query", mode, "SELECT LCase(NULL),UCase(NULL)"]);
        assert!(result.status.success());
        assert_eq!(row(&result.stdout), ["NULL", "NULL"]);
    }
}
#[test]
fn invalid_calls_and_locale_options_do_not_publish_rows() {
    for mode in ["eval", "run"] {
        for expression in [
            "LCase(1)",
            "UCase()",
            "LCase('x',1/0)",
            "UCase(NULL,1/0)",
            "VBA.LCase('x')",
            "UCase(*)",
            "IIf(False,LCase(1/0),'x')",
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--case-locale",
                "en-US",
            ]);
            assert!(!result.status.success(), "{mode} {expression}");
            assert!(result.stdout.is_empty());
        }
        for locale in ["en_US", "en-US-u-co-phonebk", ""] {
            let result = run(&["query", mode, "SELECT 1", "--case-locale", locale]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
}
