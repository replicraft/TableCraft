//! The explicit blank command publishes through the ordinary creation report path.
use super::*;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn blank(path: &Path) -> Arguments {
    let parsed =
        Cli::try_parse_from(["tablecraft", "create", path.to_str().unwrap(), "--blank"]).unwrap();
    let Some(Command::Create(arguments)) = parsed.command else {
        panic!("expected blank create command");
    };
    arguments
}

#[test]
fn explicit_blank_publishes_the_complete_independent_image_and_zero_counts() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("empty database.accdb");
    let mut report = Vec::new();
    execute(blank(&path), None, &mut report).unwrap();
    assert_eq!(fs::read(&path).unwrap(), canonical_blank_creation::source());
    assert_eq!(
        report,
        b"Database\tCreated ACE 2016\nColumns\t0\nRows\t0\nDurable output\tFile and captured-directory barriers completed\n",
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);

    assert!(matches!(
        execute(blank(&path), None, &mut Vec::new()),
        Err(Error::Creation(CreationError::Publication(
            PublicationError::NotPublished { .. }
        ))),
    ));
    assert_eq!(fs::read(path).unwrap(), canonical_blank_creation::source());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn blank_password_refusal_precedes_destination_capture_and_publication() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("must-not-exist.accdb");
    let mut report = Vec::new();
    assert!(matches!(
        execute(blank(&path), Some("not-supported"), &mut report),
        Err(Error::Password),
    ));
    assert!(report.is_empty());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
