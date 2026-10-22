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
