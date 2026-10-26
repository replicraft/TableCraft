mod support;

use std::path::Path;
use support::run;

#[test]
fn unknown_arithmetic_composes_with_union_conversion_in_both_cli_profiles() {
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/Access_Example_VBA.accdb");
    for native in [false, true] {
        let source = if native { " FROM ROULETTE_TB" } else { "" };
        let sql = format!(
            "SELECT V,VarType(V) AS T FROM \
             (SELECT First(CInt(1))+CInt(2) AS V{source} UNION ALL SELECT CCur(4)) Q"
        );
        for declared in [false, true] {
            let mut args = vec!["query", "run", &sql, "--binding-depth", "32"];
            if native {
                args.extend(["--database", example.to_str().unwrap()]);
            }
            if declared {
                args.push("--declared-arithmetic");
            }
            let result = run(&args);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let expected: &[u8] = if declared {
                b"V\tT\n3.0000\t6\n4.0000\t6\n"
            } else {
                b"V\tT\n3\t2\n4.0000\t6\n"
            };
            assert_eq!(result.stdout, expected);
        }
    }
}

#[test]
fn union_conversion_never_permits_unknown_widening_or_partial_stdout() {
    let result = run(&[
        "query",
        "run",
        "SELECT First(CInt(32767))+CInt(1) AS V UNION ALL SELECT CCur(4)",
        "--declared-arithmetic",
        "--binding-depth",
        "32",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("declaration is unknown"));
}
