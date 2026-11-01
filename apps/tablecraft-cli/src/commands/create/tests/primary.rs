//! Argument and pre-input selector checks for the real create dispatch.
use super::super::{Arguments, Error, execute};

use crate::{Cli, commands::Command};
use clap::Parser;
use std::{fs, path::Path};
use tablecraft_access::creation::CreationError;
use tablecraft_ace::CreateDatabaseError;

fn words(path: &Path) -> Vec<String> {
    vec![
        "tablecraft".into(),
        "create".into(),
        path.to_str().unwrap().into(),
        "--table".into(),
        "Items".into(),
        "--column".into(),
        "Id".into(),
    ]
}

fn parsed(words: Vec<String>) -> Arguments {
    let Command::Create(args) = Cli::try_parse_from(words).unwrap().command.unwrap() else {
        panic!("actual create command required");
    };
    args
}

#[test]
fn primary_flag_requires_one_complete_column_and_name_pair() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("new.accdb");
    for extra in [
        vec!["--primary-key"],
        vec!["--primary-key", "Id"],
        vec![
            "--primary-key",
            "Id",
            "PrimaryId",
            "--primary-key",
            "Id",
            "Other",
        ],
    ] {
        let mut input = words(&path);
        input.extend(extra.into_iter().map(str::to_owned));
        assert!(Cli::try_parse_from(input).is_err());
        assert!(!path.exists());
    }
    let mut input = words(&path);
    input.extend(["--primary-key", "id", "PrimaryId"].map(str::to_owned));
    let args = parsed(input);
    assert_eq!(
        args.primary_key,
        vec!["id".to_owned(), "PrimaryId".to_owned()]
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn primary_selector_and_unsupported_kind_refuse_before_destination_or_data_capture() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("missing-parent/new.accdb");
    let missing = root.path().join("missing.tsv");
    let mut input = words(&path);
    input.extend([
        "--primary-key".into(),
        "Missing".into(),
        "PrimaryId".into(),
        "--data".into(),
        missing.to_str().unwrap().into(),
    ]);
    assert!(matches!(
        execute(parsed(input), None, &mut Vec::new()),
        Err(Error::Input(
            "Primary-key column selector does not match a declared column"
        )),
    ));

    let input = vec![
        "tablecraft".into(),
        "create".into(),
        path.to_str().unwrap().into(),
        "--table".into(),
        "Items".into(),
        "--field".into(),
        "Id".into(),
        "byte".into(),
        "--primary-key".into(),
        "Id".into(),
        "PrimaryId".into(),
        "--data".into(),
        missing.to_str().unwrap().into(),
    ];
    assert!(matches!(
        execute(parsed(input), None, &mut Vec::new()),
        Err(Error::Creation(CreationError::Create(
            CreateDatabaseError::Declaration
        ))),
    ));
    let mut duplicate = words(&path);
    duplicate.extend([
        "--column".into(),
        "id".into(),
        "--primary-key".into(),
        "Id".into(),
        "PrimaryId".into(),
        "--data".into(),
        missing.to_str().unwrap().into(),
    ]);
    assert!(matches!(
        execute(parsed(duplicate), None, &mut Vec::new()),
        Err(Error::Input(
            "Primary-key column selector matches more than one declaration"
        )),
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
