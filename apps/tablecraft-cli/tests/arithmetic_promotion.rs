use std::process::Command;

#[test]
fn scalar_and_bound_commands_publish_promoted_types_and_buffer_late_overflow() {
    for mode in ["eval", "run"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query",mode,"SELECT VarType(CByte(1)+CByte(2)) AS B,VarType(CInt(32767)+CInt(1)) AS I,VarType(CSng(1)+CInt(1)) AS S,CCur(1.5)*CCur(1.5) AS C"])
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"B\tI\tS\tC\n17\t3\t4\t2.2500\n");
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT CByte(1)+CByte(1),CLngLng(9223372036854775807)+CByte(1)",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
