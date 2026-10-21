use super::*;

#[test]
fn inline_punctuation_prompts_bind_as_names_instead_of_field_or_control_paths() {
    for (name, binding) in [
        ("Enter No. of items", "Enter No. of items=long:2"),
        ("P.Q", "[P.Q]=long:2"),
        ("Ask!Now", "[Ask!Now]=long:2"),
        ("Floor>= minimum", "[Floor>= minimum]=long:2"),
        ("NeverRunMe()", "NeverRunMe()=long:2"),
    ] {
        let sql = format!("PARAMETERS [{name}] LONG; SELECT [{name}] AS Chosen");
        let result = cli_support::run(&["query", "run", &sql, "--param", binding]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Chosen\n2\n");
    }
}

#[test]
fn bracketed_assignment_names_preserve_text_values_and_reject_ambiguous_or_malformed_bindings() {
    for (name, binding) in [("P", "P=text:a=b:c"), ("P=Q", "[P=Q]=text:a=b:c")] {
        let sql = format!("PARAMETERS [{name}] TEXT; SELECT [{name}] AS Chosen");
        let result = cli_support::run(&["query", "run", &sql, "--param", binding]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Chosen\n\"\"\"a=b:c\"\"\"\n");
    }
    let sql = "PARAMETERS [P.Q] LONG; SELECT [P.Q] AS Chosen";
    for bindings in [
        vec!["P.Q=long:1", "[p.q]=long:2"],
        vec!["[P.Q=long:1"],
        vec!["[P.Q]junk=long:1"],
        vec!["[P[Q]]=long:1"],
        vec!["[P.Q]=long:no"],
    ] {
        let mut args = vec!["query", "run", sql];
        for binding in bindings {
            args.extend(["--param", binding]);
        }
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn native_punctuation_prompts_run_saved_and_named_and_never_prompt_while_inspecting() {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    let db = Database::new(fixture::database_with_query_flags(
        &[
            Fragment {
                name1: Some("Enter No. of items"),
                flag: Some(4),
                ..row(2)
            },
            Fragment {
                name1: Some("Items"),
                ..row(5)
            },
            Fragment {
                expression: Some("Id"),
                name1: Some("Chosen"),
                ..row(6)
            },
            Fragment {
                expression: Some("Id>=[Enter No. of items]"),
                ..row(8)
            },
        ],
        0,
    ));
    let inert = cli_support::run(&["query", "saved", "--database", db.path(), "Saved"]);
    assert!(inert.status.success());
    for query in [vec!["--saved", "Saved"], vec!["SELECT * FROM Saved"]] {
        let mut args = vec!["query", "run"];
        args.extend(query);
        args.extend([
            "--database",
            db.path(),
            "--param",
            "[Enter No. of items]=long:2",
        ]);
        let result = cli_support::run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Chosen\n2\n3\n");
    }
    let missing = cli_support::run(&["query", "run", "--database", db.path(), "--saved", "Saved"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
}
