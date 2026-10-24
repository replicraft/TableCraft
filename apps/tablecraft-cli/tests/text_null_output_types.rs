mod support;
use support::run;

#[test]
fn null_text_union_arms_keep_the_other_arms_byte_carrier() {
    for expression in [
        "Len(NULL)",
        "LenB(NULL)",
        "LCase(NULL)",
        "Trim(NULL)",
        "Left(NULL,1)",
        "LeftB(NULL,1)",
        "StrConv(NULL,1)",
        "Space(NULL)",
        "String(1,NULL)",
        "Hex(NULL)",
        "StrComp('a',NULL,0)",
        "InStr(1,NULL,'a',0)",
        "InStrB(1,'a',NULL,0)",
        "InStrRev('a',NULL,-1,0)",
    ] {
        let sql = format!(
            "SELECT N,VarType(N) AS Kind FROM (SELECT {expression} AS N UNION ALL SELECT CByte(2)) AS Q"
        );
        let result = run(&["query", "run", &sql]);
        assert!(
            result.status.success(),
            "{expression}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\tKind\nNULL\t1\n2\t17\n", "{expression}");
    }
}

#[test]
fn null_text_metadata_does_not_allow_partial_output_before_a_late_failure() {
    for expression in [
        "LeftB(NULL,-1)",
        "StrComp(NULL,'a',-2)",
        "Left(NULL,1/0)",
        "StrConv(NULL,1/0)",
        "String(NULL,1/0)",
    ] {
        let sql = format!("SELECT CByte(2) AS N UNION ALL SELECT {expression}");
        let result = run(&["query", "run", &sql]);
        assert_eq!(result.status.code(), Some(2), "{expression}");
        assert!(result.stdout.is_empty(), "{expression}");
    }
}
