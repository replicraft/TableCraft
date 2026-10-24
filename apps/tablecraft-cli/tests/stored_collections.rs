mod support;

#[test]
fn explicit_attachment_allowances_require_the_decoded_native_view_and_valid_sizes() {
    for args in [
        vec!["query", "run", "SELECT 1", "--attachment-bytes", "3145728"],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--database",
            "missing.accdb",
            "--attachment-bytes",
            "3145728",
        ],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--database",
            "missing.accdb",
            "--decoded-attachments",
            "--attachment-bytes",
            "0",
        ],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--database",
            "missing.accdb",
            "--decoded-attachments",
            "--attachment-bytes",
            "16777217",
        ],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--sqlite",
            "missing.sqlite",
            "--attachment-bytes",
            "3145728",
        ],
    ] {
        let result = support::run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("--attachment-bytes"));
    }
}

#[test]
fn stored_collection_modes_require_one_explicit_table_and_exclude_other_modes() {
    for option in ["--attachments", "--version-history"] {
        let help = support::run(&["inspect", "--help"]);
        assert!(String::from_utf8(help.stdout).unwrap().contains(option));
        let missing = support::run(&["inspect", "missing.accdb", option, "Field"]);
        assert_eq!(missing.status.code(), Some(2));
        assert!(missing.stdout.is_empty());
        assert!(String::from_utf8_lossy(&missing.stderr).contains("--table"));
        for mode in [
            "--indexes",
            "--properties",
            "--features",
            "--complex",
            "--check",
            "--constraints",
            "--relationships",
            "--catalog",
        ] {
            let result = support::run(&[
                "inspect",
                "missing.accdb",
                "--table",
                "Items",
                option,
                "Field",
                mode,
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains("cannot be used"));
        }
        for (mode, value) in [
            ("--query", "Name"),
            ("--complex-links", "Field"),
            ("--stored-calculation", "Field"),
            ("--attachments", "Other"),
            ("--version-history", "Other"),
        ] {
            let result = support::run(&[
                "inspect",
                "missing.accdb",
                "--table",
                "Items",
                option,
                "Field",
                mode,
                value,
            ]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
    }
}
