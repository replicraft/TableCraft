use super::*;

#[test]
fn saved_native_key_order_controls_headers_and_rows_without_rewriting_inert_inspection() {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    let db = Database::new(fixture::database_with_query_flags(
        &[
            Fragment {
                expression: Some("Id+100"),
                name1: Some("KeyB"),
                order: Some(&[0, 0, 0, 2]),
                ..row(6)
            },
            Fragment {
                expression: Some("Id"),
                name1: Some("KeyA"),
                ..row(6)
            },
            Fragment {
                name1: Some("Items"),
                ..row(5)
            },
        ],
        0,
    ));
    let report = cli_support::run(&["inspect", db.path(), "--query", "Saved"]);
    assert!(report.status.success());
    let report = String::from_utf8(report.stdout).unwrap();
    assert!(report.find("\"Id+100\"").unwrap() < report.find("\"Id\"").unwrap());
    for query in [vec!["--saved", "Saved"], vec!["SELECT * FROM Saved"]] {
        let mut args = vec!["query", "run"];
        args.extend(query);
        args.extend(["--database", db.path()]);
        let result = cli_support::run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"KeyA\tKeyB\n1\t101\n2\t102\n3\t103\n");
    }
}
