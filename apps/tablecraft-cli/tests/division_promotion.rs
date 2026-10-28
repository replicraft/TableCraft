use std::process::Command;

#[test]
fn both_commands_publish_unary_and_division_types_and_buffer_operand_failures() {
    for mode in ["eval", "run"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query",mode,"SELECT VarType(-CByte(1)) AS U,VarType(CSng(3)/CInt(2)) AS S,VarType(CByte(9)\\CByte(2)) AS B,CCur(3)/CCur(2) AS Q"])
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"U\tS\tB\tQ\n2\t4\t17\t1.5\n");
        for sql in ["SELECT 1,CInt(9)\\'32767.5'", "SELECT 1,CCur(1)/CCur(0)"] {
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", mode, sql, "--number-format", "en-us"])
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn zero_quotients_report_the_selected_error_without_partial_output() {
    for (mode, declared) in [("eval", false), ("run", false), ("run", true)] {
        for (expression, message) in [
            ("CSng(0)/CSng(-0.0)", "numeric overflow"),
            ("CCur(0)/CDate(0)", "numeric overflow"),
            ("CDec(0)/CDbl(0)", "division by zero"),
            ("0\\CDbl(0.5)", "division by zero"),
            ("0 Mod CDbl(0.5)", "division by zero"),
            ("IsError(0/0)", "numeric overflow"),
        ] {
            // Declared arithmetic promotes this Decimal/Double pair before
            // division; its floating 0/0 result is overflow, unlike Variant mode.
            let message = if declared && expression == "CDec(0)/CDbl(0)" {
                "numeric overflow"
            } else {
                message
            };
            let sql = format!("SELECT 42 AS BeforeFailure,{expression}");
            let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
            command.args(["query", mode, &sql]);
            if declared {
                command.arg("--declared-arithmetic");
            }
            let output = command.output().unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(message),
                "mode={mode}, declared={declared}, expression={expression}: {}",
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }
}
