use std::process::Command;

#[test]
fn cli_declares_unsecured_access_session_without_using_the_os_account() {
    for mode in ["eval", "run"] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                mode,
                "SELECT CurrentUser(),Eval('CurrentUser()'),TypeName(CurrentUser())",
            ])
            .env("USER", "ignored-account")
            .env("LOGNAME", "ignored-account")
            .env("USERNAME", "ignored-account")
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
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            ["\"Admin\"", "\"Admin\"", "\"String\""]
        );
        for sql in [
            "SELECT CurrentUser(1/0)",
            "SELECT CurrentUser(*)",
            "SELECT CurrentUser(),1/0",
        ] {
            let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", mode, sql])
                .output()
                .unwrap();
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
}
