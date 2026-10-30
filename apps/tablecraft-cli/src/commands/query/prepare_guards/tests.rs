use super::*;
use clap::Parser;

#[derive(Parser)]
struct TestCommand {
    #[command(flatten)]
    args: Arguments,
}

#[test]
fn guard_sql_shape_refusals_precede_missing_source_open() {
    for sql in [
        "SELECT * FROM Items",
        "UPDATE Items SET Score=11,Id=4",
        "UPDATE Items IN 'remote.accdb' SET Score=11",
        "INSERT INTO Items SELECT Id,Score FROM Other",
    ] {
        let parsed = TestCommand::try_parse_from([
            "guard",
            sql,
            "--database",
            "missing-guard-source.accdb",
            "--guard-property",
            "3",
            "StoredGuard",
        ])
        .unwrap();
        assert!(matches!(
            run(parsed.args, None),
            Err(Error::Query(_)) | Err(Error::Parse(_))
        ));
    }
    assert!(
        TestCommand::try_parse_from([
            "guard",
            "DELETE FROM Items",
            "--database",
            "missing.accdb",
            "--guard-property",
            "3",
            "StoredGuard",
            "--output",
            "never.accdb"
        ])
        .is_err()
    );
}

#[test]
fn guard_property_pairs_are_explicit_bounded_and_keep_caller_order() {
    assert!(
        TestCommand::try_parse_from(["guard", "DELETE FROM Items", "--database", "missing.accdb"])
            .is_err()
    );
    assert!(
        TestCommand::try_parse_from([
            "guard",
            "DELETE FROM Items",
            "--database",
            "missing.accdb",
            "--guard-property",
            "3"
        ])
        .is_err()
    );
    let parsed = TestCommand::try_parse_from([
        "guard",
        "DELETE FROM Items",
        "--database",
        "missing.accdb",
        "--guard-property",
        "3",
        "GuardB",
        "--guard-property",
        "3",
        "GuardA",
    ])
    .unwrap();
    let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
    let selected = selectors(&parsed.args.guard_properties, &mut budget)
        .unwrap_or_else(|_| panic!("valid selector pairs"));
    assert_eq!(
        selected
            .iter()
            .map(|value| (value.group(), value.name()))
            .collect::<Vec<_>>(),
        [(3, "GuardB"), (3, "GuardA")]
    );
    let invalid = TestCommand::try_parse_from([
        "guard",
        "DELETE FROM Items",
        "--database",
        "missing.accdb",
        "--guard-property",
        "bad",
        "GuardA",
    ])
    .unwrap();
    assert!(matches!(run(invalid.args, None), Err(Error::Selection)));
}
