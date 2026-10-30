//! Publish complete bytes under a fresh name; failures clean only owned temporaries.
use std::{fmt, fs::File, io::ErrorKind, path::Path};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::publication::{PublicationError, RequiredOutput};
use tempfile::NamedTempFile;

const COPY_CHUNK_BYTES: usize = 64 * 1024;
pub(crate) const REQUIRED_REPORT: &str =
    "Durable output\tFile and captured-directory barriers completed\n";

#[derive(Debug)]
pub(crate) enum Error {
    Create(ErrorKind),
    Write(tablecraft_io::IoError),
    Sync(ErrorKind),
    Persist(ErrorKind),
    Native(InspectError),
    Required(PublicationError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create(kind) => {
                write!(f, "could not create an owned temporary output ({kind:?})")
            }
            Self::Write(error) => write!(
                f,
                "database copy write failed; no final output was published: {error}"
            ),
            Self::Sync(kind) => write!(
                f,
                "database copy sync failed; no final output was published ({kind:?})"
            ),
            Self::Persist(kind) => write!(
                f,
                "could not publish a fresh database copy; existing destinations are never overwritten ({kind:?})"
            ),
            Self::Native(error) => error.fmt(f),
            Self::Required(error) => error.fmt(f),
        }
    }
}

/// Select publication once, retaining a required captured parent before source I/O.
pub(crate) enum Output<'a> {
    ContentOnly(&'a Path),
    Required(RequiredOutput),
}

impl<'a> Output<'a> {
    pub(crate) fn capture(
        path: &'a Path,
        required: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        if required {
            RequiredOutput::capture(path, budget, cancelled)
                .map(Self::Required)
                .map_err(Error::Required)
        } else {
            Ok(Self::ContentOnly(path))
        }
    }

    pub(crate) fn publish(
        self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        match self {
            Self::ContentOnly(path) => publish(path, bytes, budget, cancelled),
            Self::Required(output) => {
                // IO owns the payload Bytes charge and the complete terminal
                // publication/barrier/cleanup protocol. Never downgrade/retry.
                let receipt = output
                    .publish(bytes, budget, cancelled)
                    .map_err(Error::Required)?;
                tracing::debug!(
                    namespace_synced = receipt.namespace_synced(),
                    "required output completed"
                );
                Ok(())
            }
        }
    }
}

trait Publication {
    fn write(
        &self,
        file: &mut File,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error>;
    fn sync(&self, file: &mut File) -> Result<(), Error>;
    fn persist(&self, file: NamedTempFile, output: &Path) -> Result<(), Error>;
}

struct Filesystem;

impl Publication for Filesystem {
    fn write(
        &self,
        file: &mut File,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        for (position, chunk) in bytes.chunks(COPY_CHUNK_BYTES).enumerate() {
            tablecraft_io::write_exact_at(
                file,
                (position * COPY_CHUNK_BYTES) as u64,
                chunk,
                budget,
                cancelled,
            )
            .map_err(Error::Write)?;
        }
        Ok(())
    }

    fn sync(&self, file: &mut File) -> Result<(), Error> {
        file.sync_all().map_err(|error| Error::Sync(error.kind()))
    }

    fn persist(&self, file: NamedTempFile, output: &Path) -> Result<(), Error> {
        file.persist_noclobber(output)
            .map(|_| ())
            .map_err(|error| Error::Persist(error.error.kind()))
    }
}

pub(crate) fn publish(
    output: &Path,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.native.copy.publish", bytes = bytes.len()).entered();
    publish_with(output, bytes, budget, cancelled, &Filesystem)
}

fn publish_with(
    output: &Path,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    io: &impl Publication,
) -> Result<(), Error> {
    if cancelled() {
        return Err(Error::Native(InspectError::Cancelled));
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    // The bounded writer charges attempts, but its caller owns requested Bytes.
    // Refuse an insufficient output allowance before creating any temporary.
    budget
        .charge(Resource::Bytes, bytes.len() as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|error| Error::Create(error.kind()))?;
    io.write(temporary.as_file_mut(), bytes, budget, cancelled)?;
    io.sync(temporary.as_file_mut())?;
    if cancelled() {
        return Err(Error::Native(InspectError::Cancelled));
    }
    io.persist(temporary, output)
}

#[cfg(test)]
mod tests;
