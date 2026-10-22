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
fn strconv_casing_and_raw_string_encoding_work_in_both_query_commands() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT StrConv('hELLO wORLD',3,1033) AS P,StrConv(ChrB(128),64,1033) AS U,LenB(StrConv('é€',128,1033)) AS B,VarType(StrConv('€',128,1033)) AS T",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(row(&result.stdout), ["\"Hello World\"", "\"€\"", "2", "8"]);
        let result = run(&[
            "query",
            mode,
            "SELECT StrConv('i',1,1033) AS E,StrConv('i',1) AS T,AscB(StrConv('€',128,1033)) AS W,AscB(StrConv('€',128)) AS C",
            "--case-locale",
            "tr",
            "--character-code-page",
            "1251",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(row(&result.stdout), ["\"I\"", "\"İ\"", "128", "136"]);
    }
}

#[test]
fn strconv_refusals_never_publish_partial_rows() {
    for mode in ["eval", "run"] {
        for expression in [
            "StrConv('x',1)",
            "StrConv('x',4,1033)",
            "StrConv('x',1,1041)",
            "StrConv('€',128,1033)",
            "StrConv('🦀',128,1033)",
            "StrConv(Null,1,1/0)",
            "StrConv('x',1,1033,1/0)",
            "StrConv(,1)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
