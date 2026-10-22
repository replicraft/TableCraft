use std::process::Command;

#[test]
fn both_query_commands_evaluate_radix_literals_and_buffer_overflow() {
    for mode in ["eval", "run"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT &HFF AS H,&HFFFF AS S,&O77 AS O,VarType(&H10000) AS T",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"H\tS\tO\tT\n255\t-1\t63\t3\n");
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", mode, "SELECT 1,&H100000000"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
