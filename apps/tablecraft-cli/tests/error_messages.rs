use std::process::Command;

#[test]
fn explicit_numbered_messages_are_buffered_with_a_declared_english_catalog() {
    for mode in ["eval", "run"] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", mode, "SELECT Error(0),Error(11),Error$(5),Error(65535),Eval('Error(13)'),TypeName(Error(6))"])
            .output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            [
                "\"\"",
                "\"Division by zero\"",
                "\"Invalid procedure call or argument\"",
                "\"Application-defined or object-defined error\"",
                "\"Type mismatch\"",
                "\"String\"",
            ]
        );
        for sql in [
            "SELECT Error()",
            "SELECT Error(11),1/0",
            "SELECT Error(NULL)",
            "SELECT Error(-1)",
            "SELECT Error(65536)",
        ] {
            let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", mode, sql])
                .output()
                .unwrap();
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty(), "{sql}");
        }
    }
}
