//! Repeated names select only named INSERT transport; KIND/DATA remains shared.
use super::{Cli, Parser, command};
use crate::commands::write::args::Command;

#[test]
fn repeated_fields_retain_written_order_and_generation_conflicts() {
    let arguments = [
        "tablecraft",
        "write",
        "preview-insert",
        "before.accdb",
        "after.accdb",
        "Items",
        "rows.tsv",
        "--field",
        "Other",
        "--field",
        "Id",
        "--incrementing",
    ];
    let Command::Insert(insert) = command(&arguments) else {
        panic!("INSERT command");
    };
    assert_eq!(insert.fields, ["Other", "Id"]);
    assert!(insert.incrementing);
    let mut conflict = arguments.to_vec();
    conflict.extend(["--incrementing-last", "30"]);
    assert!(Cli::try_parse_from(conflict).is_err());
}
