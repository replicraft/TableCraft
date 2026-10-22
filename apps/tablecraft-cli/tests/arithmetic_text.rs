use std::process::Command;

#[test]
fn both_query_commands_use_explicit_arithmetic_text_policy_and_buffer_failures() {
    for mode in ["eval", "run"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT '2,5'+3 AS A,'2'+'3' AS S,'9,5' Mod '2,5' AS M,-'2,5' AS N",
                "--number-format",
                "de-de",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"A\tS\tM\tN\n5.5\t\"\"\"23\"\"\"\t0\t-2.5\n");
        for sql in ["SELECT 1,'bad'+2", "SELECT 1,'2'/0", "SELECT Null+('2'/0)"] {
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", mode, sql, "--number-format", "en-us"])
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
    }
}
