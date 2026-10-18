mod support;
use support::run;

#[test]
fn help_states_actual_capabilities_without_a_display() {
    for args in [&[][..], &["--help"][..], &["-h"][..]] {
        let output = run(args);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Usage: tablecraft"));
        assert!(text.contains("Query update creates a fresh offline Jet 4 copy"));
        assert!(text.contains("In-place CLI editing and GUI hosts are not implemented."));
    }
}

#[test]
fn version_matches_package_metadata() {
    for flag in ["--version", "-V"] {
        let output = run(&[flag]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            format!("tablecraft {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
    }
}

#[test]
fn unsupported_commands_and_extra_arguments_fail_without_success_output() {
    for args in [
        &["edit", "example.accdb"][..],
        &["--unknown"][..],
        &["unexpected", "extra"][..],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("error:"));
    }
}

#[test]
fn query_parse_prints_owned_normalized_sql_without_a_database() {
    let output = run(&[
        "query",
        "parse",
        "PARAMETERS [Minimum score] Long; SELECT Code FROM Samples WHERE Code > [Minimum score]",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "PARAMETERS [Minimum score] LONG; SELECT [Code] FROM [Samples] WHERE ([Code] > [Minimum score]);\n"
    );
    let output = run(&["query", "parse", "UPDATE Samples SET Code = 1"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "UPDATE [Samples] SET [Code] = 1;\n"
    );
}
#[test]
fn query_parse_rejects_invalid_sql_and_limits_without_partial_output() {
    for sql in [
        "SELECT * FROM",
        "SELECT * FROM Samples; DELETE FROM Samples",
        &"x".repeat(65_537),
    ] {
        let output = run(&["query", "parse", sql]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .starts_with("error:")
        );
    }
    let help = run(&["query", "parse", "--help"]);
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("does not execute")
    );
}
