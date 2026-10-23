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

#[test]
fn signed_numbers_are_messages_and_positive_range_failures_are_buffered() {
    for mode in ["eval", "run"] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT Error(-1),Error$(-2147483648),Error(True)",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout)
                .matches("Application-defined or object-defined error")
                .count(),
            3
        );
        let overflow = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", mode, "SELECT Error(-1),Error(65536)"])
            .output()
            .unwrap();
        assert_eq!(overflow.status.code(), Some(2));
        assert!(overflow.stdout.is_empty());
        assert!(String::from_utf8_lossy(&overflow.stderr).contains("overflow"));
    }
}

#[test]
fn error_dollar_retains_scalar_string_category_in_the_explicit_profile() {
    for flag in ["--declared-arithmetic", "--declared-integral"] {
        let variant = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT CVar(1)<Error(-1) AS N", flag])
            .output()
            .unwrap();
        assert!(variant.status.success());
        assert_eq!(variant.stdout, b"N\nTRUE\n");
        let scalar = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT 1,CVar(1)<Error$(-1)", flag])
            .output()
            .unwrap();
        assert_eq!(scalar.status.code(), Some(2));
        assert!(scalar.stdout.is_empty());
        assert!(String::from_utf8_lossy(&scalar.stderr).contains("number-format"));
    }
}

#[test]
fn numbered_message_error_state_needs_explicit_conversion_and_keeps_eager_failure() {
    for mode in ["eval", "run"] {
        for function in ["Error", "Error$"] {
            for (argument, diagnostic) in [
                ("CVErr(11)", "type mismatch"),
                ("IIf(True,CVErr(11),1/0)", "division by zero"),
            ] {
                let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                    .args(["query", mode, &format!("SELECT 1,{function}({argument})")])
                    .output()
                    .unwrap();
                assert_eq!(result.status.code(), Some(2));
                assert!(result.stdout.is_empty());
                assert!(String::from_utf8_lossy(&result.stderr).contains(diagnostic));
            }
            let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args([
                    "query",
                    mode,
                    &format!("SELECT {function}(CLng(CVErr(11))) AS M"),
                ])
                .output()
                .unwrap();
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
            assert_eq!(row.get(0), Some("\"Division by zero\""));
        }
    }
}
