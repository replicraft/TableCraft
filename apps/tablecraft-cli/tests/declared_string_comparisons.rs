#[path = "support/mod.rs"]
mod support;

use std::process::Output;
use support::run;

fn query(sql: &str, declared: bool) -> Output {
    let mut arguments = vec![
        "query",
        "run",
        sql,
        "--collation",
        "ordinal",
        "--number-format",
        "invariant",
    ];
    if declared {
        arguments.push("--declared-arithmetic");
    }
    run(&arguments)
}

#[test]
fn scalar_strings_and_variant_numbers_use_text_only_in_the_declared_profile() {
    let sql = "SELECT '02'=CVar(CLng(2)) AS Eq, '02'<>CVar(CLng(2)) AS Ne, \
               '02'<CVar(CLng(2)) AS Lt, '02'<=CVar(CLng(2)) AS Le, \
               '02'>CVar(CLng(2)) AS Gt, '02'>=CVar(CLng(2)) AS Ge";
    for (declared, expected) in [
        (
            false,
            &b"Eq\tNe\tLt\tLe\tGt\tGe\nTRUE\tFALSE\tFALSE\tTRUE\tFALSE\tTRUE\n"[..],
        ),
        (
            true,
            &b"Eq\tNe\tLt\tLe\tGt\tGe\nFALSE\tTRUE\tTRUE\tTRUE\tFALSE\tFALSE\n"[..],
        ),
    ] {
        let result = query(sql, declared);
        assert!(result.status.success(), "{:?}", result);
        assert_eq!(result.stdout, expected);
        assert!(result.stderr.is_empty());
    }

    let reverse = query(
        "SELECT CVar(CLng(2))>'02' AS Reverse, '10'<CVar(CLng(2)) AS Lexical, \
         'bad'<CVar(CLng(2)) AS Nonnumeric",
        true,
    );
    assert!(reverse.status.success(), "{:?}", reverse);
    assert_eq!(
        reverse.stdout,
        b"Reverse\tLexical\tNonnumeric\nTRUE\tTRUE\tFALSE\n"
    );
}

#[test]
fn both_variant_ranking_numeric_scalar_coercion_and_null_remain_separate() {
    let result = query(
        "SELECT CVar('02')<CVar(CLng(2)) AS Ranked, \
         CLng(2)=CVar('02') AS [Numeric], '02'=CVar('2') AS Textual, \
         '02'=CVar(Null) AS [Unknown]",
        true,
    );
    assert!(result.status.success(), "{:?}", result);
    assert_eq!(
        result.stdout,
        b"Ranked\tNumeric\tTextual\tUnknown\nFALSE\tTRUE\tFALSE\tNULL\n"
    );

    let result = query(
        "SELECT '10' BETWEEN CVar(CLng(1)) AND CVar(CLng(2)) AS InRange, \
         '02' IN (CVar(CLng(2)),CVar(CLng(3))) AS Member",
        true,
    );
    assert!(result.status.success(), "{:?}", result);
    assert_eq!(result.stdout, b"InRange\tMember\nTRUE\tFALSE\n");
}

#[test]
fn declared_string_conversion_and_late_operand_errors_remain_buffered() {
    for (sql, diagnostic) in [
        ("SELECT '02'=CVar(CLng(2)), 1/0", "division by zero"),
        (
            "SELECT '02' IN (CVar(CLng(2)),CVar(1/0))",
            "division by zero",
        ),
        ("SELECT '02'=CVErr(1)", "type mismatch"),
        ("SELECT '02'=First(CLng(2))", "declaration is unknown"),
    ] {
        let result = query(sql, true);
        assert_eq!(result.status.code(), Some(2), "{:?}", result);
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains(diagnostic));
    }

    for options in [
        vec![],
        vec!["--collation", "ordinal"],
        vec![
            "--collation",
            "ordinal",
            "--number-format",
            "invariant",
            "--execution-steps",
            "1",
        ],
        vec![
            "--collation",
            "ordinal",
            "--number-format",
            "invariant",
            "--output-bytes",
            "1",
        ],
    ] {
        let mut arguments = vec![
            "query",
            "run",
            "SELECT '02'=CVar(CLng(2)) AS Compared",
            "--declared-arithmetic",
        ];
        arguments.extend(options);
        let result = run(&arguments);
        assert_eq!(result.status.code(), Some(2), "{:?}", result);
        assert!(result.stdout.is_empty());
    }
}
