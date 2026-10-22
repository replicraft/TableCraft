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
fn date_masks_names_week_fields_and_intrinsic_formatting_work_in_both_commands() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Format(#1993-01-27 17:04:23#,'yyyy-mm-dd') AS D,Format(#1993-01-27 17:04:23#,'hh:mm:ss am/pm') AS T,Format(#2018-12-31#,'w ww',2,2) AS W",
            "--date-format",
            "en-US",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            row(&result.stdout),
            ["\"1993-01-27\"", "\"05:04:23 pm\"", "\"1 53\""]
        );
        let result = run(&[
            "query",
            mode,
            "SELECT Format(#1993-01-27#,'mmmm') AS M",
            "--date-format",
            "de-DE",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(row(&result.stdout), ["\"Januar\""]);
        let result = run(&[
            "query",
            mode,
            "SELECT Format(#1993-01-27#,'yyyy-mm-dd') AS D",
        ]);
        assert!(result.status.success());
        assert_eq!(row(&result.stdout), ["\"1993-01-27\""]);
    }
}

#[test]
fn date_format_errors_publish_no_partial_stdout() {
    for mode in ["eval", "run"] {
        for expression in [
            "Format(#1993-01-27#,'dddd')",
            "Format(#1993-01-27#,'m/d/yyyy')",
            "Format(1,'yyyyy')",
            "Format(1,'yyyy;yyyy')",
            "Format(Null,'yyyy',1/0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
