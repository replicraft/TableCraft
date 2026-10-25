mod support;

use support::run;

#[test]
fn strconv_expression_state_failures_stay_eager_and_buffered() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT LenB(StrConv(CVar(MidB('😀',1,1)),0)),StrConv(CStr(CVErr(11)),2,1033)",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(row(&result.stdout), ["1", "\"error 11\""]);
        for (expression, diagnostic) in [
            ("StrConv(CVar(CVErr(11)),-1)", "domain"),
            ("StrConv(CVErr(11),0,0)", "domain"),
            ("StrConv(NULL,CVErr(11))", "type mismatch"),
            ("StrConv(CVErr(11),0,1/0)", "division by zero"),
        ] {
            let sql = format!("SELECT 1,{expression}");
            let result = run(&["query", mode, &sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty(), "{sql}");
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(diagnostic),
                "{sql}"
            );
        }
    }
}

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
            "StrConv('x',1,1042)",
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

#[test]
fn explicit_japanese_widening_precedes_hiragana_in_both_query_commands() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT StrConv('ｶﾞﾊﾟｳﾞ ABC',36,1041) AS S,LenB(StrConv('ｶﾞ',36,1041)) AS B",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(row(&result.stdout), ["\"がぱゔ　ＡＢＣ\"", "2"]);

        for expression in [
            "StrConv('ｶﾞ',36)",
            "StrConv('ｶﾞ',36,1033)",
            "StrConv('ｶﾞ',37,1041)",
        ] {
            let sql = format!("SELECT 1,{expression}");
            let refused = run(&["query", mode, &sql]);
            assert!(!refused.status.success());
            assert!(refused.stdout.is_empty());
        }
    }
}
