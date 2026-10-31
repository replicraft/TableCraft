use super::{Arguments, Error, run_with_release};
use crate::native::{self, publication};
use clap::Parser;
use std::{
    cell::Cell,
    fs,
    io::{self, ErrorKind, Write},
};
use tablecraft_access::AccessInputError;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::mutation_fixture;

fn arguments(sql: &str, input: &str, output: &str, kind: &str, required: bool) -> Arguments {
    let mut values = vec![
        "tablecraft",
        "query",
        "preview-before-change",
        sql,
        "--database",
        input,
        "--output",
        output,
        "--guard-property",
        kind,
        "StoredGuard",
    ];
    if required {
        values.push("--durable-output");
    }
    let cli = crate::Cli::try_parse_from(values)
        .unwrap_or_else(|error| panic!("test arguments refused: {error}"));
    match cli.command {
        Some(crate::commands::Command::Query {
            command: crate::commands::query::Command::PreviewBeforeChange(args),
        }) => *args,
        _ => panic!("wrong test command"),
    }
}

#[test]
fn closed_shape_refusal_precedes_required_capture_and_source_open() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("missing-source");
    let output = root.path().join("missing-parent/copy");
    for sql in [
        "SELECT 1",
        "UPDATE Items SET Score=12, Id=4",
        "PARAMETERS p LONG; UPDATE Items SET Score=p",
        "UPDATE [remote].[Items] SET Score=12",
        "DELETE FROM Items",
        "INSERT INTO Items (Id,Score) VALUES(4,40)",
    ] {
        let args = arguments(
            sql,
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "3",
            true,
        );
        let result = run_with_release(args, |_| panic!("preflight must not acquire source"));
        assert!(matches!(result, Err(Error::Query(_))));
        assert!(!input.exists());
        assert!(!output.exists());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[test]
fn malformed_selector_refuses_before_source_or_publication_capture() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("missing-source");
    let output = root.path().join("missing-parent/copy");
    let args = arguments(
        "UPDATE Items SET Score=12",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "not-a-kind",
        true,
    );
    assert!(matches!(
        run_with_release(args, |_| panic!(
            "malformed selector must not acquire source"
        )),
        Err(Error::Selection(_))
    ));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn before_change_command_keeps_growth_and_other_update_modes_closed() {
    for flag in [
        "--variable",
        "--long-value",
        "--index-growth",
        "--cascade-updates",
        "--image-bytes",
    ] {
        assert!(
            crate::Cli::try_parse_from([
                "tablecraft",
                "query",
                "preview-before-change",
                "UPDATE Items SET Score=12",
                "--database",
                "source",
                "--output",
                "copy",
                "--guard-property",
                "3",
                "StoredGuard",
                flag,
            ])
            .is_err()
        );
    }
    assert!(
        crate::Cli::try_parse_from([
            "tablecraft",
            "query",
            "preview-before-change",
            "UPDATE Items SET Score=12",
            "--database",
            "source",
            "--output",
            "copy",
            "--guard-property",
            "3",
            "StoredGuard",
            "--trace",
            "--collation",
            "ordinal",
            "--durable-output",
        ])
        .is_ok()
    );
}

#[test]
fn known_published_copy_write_and_flush_failures_keep_their_failure_class() {
    struct FailedOutput(bool);

    impl Write for FailedOutput {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 {
                Err(io::ErrorKind::BrokenPipe.into())
            } else {
                Ok(bytes.len())
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    for fail_write in [false, true] {
        assert!(matches!(
            super::write_published("complete prepared report\n", &mut FailedOutput(fail_write)),
            Err(crate::commands::ExecutionError::PublishedCopyOutput(error))
                if error.kind() == io::ErrorKind::BrokenPipe
        ));
    }
}

#[test]
fn source_release_failure_prevents_either_publication_mode() {
    // This tests only the release/publication boundary. Byte identity here is
    // not a BeforeChange admission claim; integration tests use the real service.
    let before = mutation_fixture::database();
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("source.mdb");
    fs::write(&input, &before).unwrap();
    for required in [false, true] {
        if required && !cfg!(any(target_os = "macos", target_os = "linux")) {
            continue;
        }
        let mut work = ResourceBudget::new(1_000_000, 0, 1_000_000);
        let source = native::open_raw(&input, &mut work).unwrap();
        let path = root.path().join("unpublished.mdb");
        let output = publication::Output::capture(&path, required, &mut work, &|| false).unwrap();
        let calls = Cell::new(0);
        let result = super::publish_prepared(
            source,
            output,
            &before,
            String::from("prepared before publication\n"),
            &mut work,
            |source| {
                calls.set(calls.get() + 1);
                source.close()?;
                Err(AccessInputError::Native(InspectError::Io(
                    ErrorKind::PermissionDenied,
                )))
            },
        );
        assert!(matches!(result, Err(Error::SourceRelease(_))));
        assert_eq!(calls.get(), 1);
        assert!(!path.exists());
        assert_eq!(fs::read(&input).unwrap(), before);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
