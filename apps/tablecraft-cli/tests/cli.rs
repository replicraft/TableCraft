use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(args)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .expect("CLI should launch without a display")
}

#[test]
fn help_states_actual_capabilities_without_a_display() {
    for args in [&[][..], &["--help"][..], &["-h"][..]] {
        let output = run(args);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Usage: tablecraft"));
        assert!(text.contains("Database inspection and editing are not implemented yet."));
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
        &["inspect", "example.accdb"][..],
        &["--unknown"][..],
        &["unexpected", "extra"][..],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("error:"));
    }
}
