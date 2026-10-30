use super::{Error, Filesystem, Publication, publish, publish_with};
use std::{
    fs::{self, File},
    io::{ErrorKind, Write},
    path::Path,
};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;
use tempfile::NamedTempFile;

#[path = "tests/required.rs"]
mod required;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 100, 1024 * 1024)
}

#[test]
fn complete_copy_never_replaces_source_or_existing_destination() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let output = directory.path().join("copy.accdb");
    fs::write(&source, b"original").unwrap();
    publish(&output, b"complete candidate", &mut budget(), &|| false).unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"complete candidate");
    for target in [&source, &output] {
        assert!(matches!(
            publish(target, b"replacement", &mut budget(), &|| false),
            Err(Error::Persist(ErrorKind::AlreadyExists))
        ));
    }
    assert_eq!(fs::read(&source).unwrap(), b"original");
    assert_eq!(fs::read(&output).unwrap(), b"complete candidate");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[derive(Clone, Copy)]
enum Fault {
    Write,
    Sync,
    Persist,
}

impl Publication for Fault {
    fn write(
        &self,
        file: &mut File,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        if matches!(self, Self::Write) {
            file.write_all(&bytes[..bytes.len().min(3)]).unwrap();
            return Err(Error::Write(tablecraft_io::IoError::Io(
                ErrorKind::WriteZero,
            )));
        }
        Filesystem.write(file, bytes, budget, cancelled)
    }

    fn sync(&self, file: &mut File) -> Result<(), Error> {
        if matches!(self, Self::Sync) {
            return Err(Error::Sync(ErrorKind::Other));
        }
        Filesystem.sync(file)
    }

    fn persist(&self, file: NamedTempFile, output: &Path) -> Result<(), Error> {
        if matches!(self, Self::Persist) {
            return Err(Error::Persist(ErrorKind::PermissionDenied));
        }
        Filesystem.persist(file, output)
    }
}

#[test]
fn every_publication_failure_cleans_only_the_owned_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let output = directory.path().join("copy.accdb");
    fs::write(&source, b"original").unwrap();
    for fault in [Fault::Write, Fault::Sync, Fault::Persist] {
        assert!(
            publish_with(
                &output,
                b"complete candidate",
                &mut budget(),
                &|| false,
                &fault
            )
            .is_err()
        );
        assert!(!output.exists());
        assert_eq!(fs::read(&source).unwrap(), b"original");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    assert!(
        publish(
            &output,
            b"candidate",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
    assert!(matches!(
        publish(&output, b"candidate", &mut budget(), &|| true),
        Err(Error::Native(InspectError::Cancelled))
    ));
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn symlink_destination_is_never_followed_or_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let output = directory.path().join("alias.accdb");
    fs::write(&source, b"original").unwrap();
    std::os::unix::fs::symlink(&source, &output).unwrap();
    assert!(matches!(
        publish(&output, b"replacement", &mut budget(), &|| false),
        Err(Error::Persist(ErrorKind::AlreadyExists))
    ));
    assert_eq!(fs::read(&source).unwrap(), b"original");
    assert!(
        fs::symlink_metadata(&output)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn final_callback_cancellation_keeps_a_complete_temporary_unpublished() {
    let directory = tempfile::tempdir().unwrap();
    let healthy = directory.path().join("healthy.accdb");
    let cancelled = directory.path().join("cancelled.accdb");
    let polls = std::cell::Cell::new(0usize);
    publish(&healthy, b"complete candidate", &mut budget(), &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let terminal = polls.get();
    assert!(terminal > 1);
    polls.set(0);
    assert!(matches!(
        publish(&cancelled, b"complete candidate", &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        }),
        Err(Error::Native(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), terminal);
    assert!(!cancelled.exists());
    assert_eq!(fs::read(&healthy).unwrap(), b"complete candidate");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn hardlink_alias_cannot_replace_a_source_inode() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let alias = directory.path().join("alias.accdb");
    fs::write(&source, b"original").unwrap();
    fs::hard_link(&source, &alias).unwrap();
    assert!(matches!(
        publish(&alias, b"replacement", &mut budget(), &|| false),
        Err(Error::Persist(ErrorKind::AlreadyExists))
    ));
    assert_eq!(fs::read(&source).unwrap(), b"original");
    assert_eq!(fs::read(&alias).unwrap(), b"original");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn broken_destination_symlink_is_an_existing_name() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("broken.accdb");
    let absent = directory.path().join("absent.accdb");
    std::os::unix::fs::symlink(&absent, &output).unwrap();
    assert!(matches!(
        publish(&output, b"replacement", &mut budget(), &|| false),
        Err(Error::Persist(ErrorKind::AlreadyExists))
    ));
    assert!(!absent.exists());
    assert!(
        fs::symlink_metadata(&output)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

struct Observed(std::cell::Cell<usize>);

impl Publication for Observed {
    fn write(
        &self,
        file: &mut File,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        self.0.set(self.0.get() + 1);
        Filesystem.write(file, bytes, budget, cancelled)
    }

    fn sync(&self, file: &mut File) -> Result<(), Error> {
        self.0.set(self.0.get() + 1);
        Filesystem.sync(file)
    }

    fn persist(&self, file: NamedTempFile, output: &Path) -> Result<(), Error> {
        self.0.set(self.0.get() + 1);
        Filesystem.persist(file, output)
    }
}

#[test]
fn output_byte_quota_precedes_temporary_creation_and_all_output_callbacks() {
    use tablecraft_core::limits::{LimitError, Resource};

    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let output = directory.path().join("copy.accdb");
    fs::write(&source, b"original").unwrap();
    let bytes = b"complete candidate";
    let required = bytes.len() as u64;
    let io = Observed(std::cell::Cell::new(0));

    for destination in [output.clone(), directory.path().join("absent/copy.accdb")] {
        let mut pool = ResourceBudget::new(required - 1, 0, 100_000);
        assert!(matches!(
            publish_with(&destination, bytes, &mut pool, &|| false, &io),
            Err(Error::Native(InspectError::Limit(LimitError {
                resource: Resource::Bytes,
                remaining,
            }))) if remaining == required - 1
        ));
        assert_eq!(pool.remaining(Resource::Bytes), required - 1);
        assert_eq!(io.0.get(), 0);
        assert!(!destination.exists());
        assert_eq!(fs::read(&source).unwrap(), b"original");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    // Exact Bytes succeeds with independent work allowance. The missing-parent
    // control above proves that denial precedes even tempfile creation.
    let mut pool = ResourceBudget::new(required, 0, 100_000);
    publish_with(&output, bytes, &mut pool, &|| false, &io).unwrap();
    assert_eq!(pool.remaining(Resource::Bytes), 0);
    assert_eq!(io.0.get(), 3);
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert_eq!(fs::read(&source).unwrap(), b"original");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}
