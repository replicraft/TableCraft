//! Read-only native input and bounded deployment-container selection.
mod deployment;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_ace::{InspectError, NativeReadSource};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

/// Input errors omit filenames, archive entry names and dependency diagnostics.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AccessInputError {
    /// Underlying native/I/O/budget failure.
    #[error(transparent)]
    Native(#[from] InspectError),
    /// Invalid CAB geometry, payload or compressed data.
    #[error("malformed Access deployment package")]
    MalformedPackage,
    /// Package needs an unsupported layout or compression scheme.
    #[error("Access deployment package layout or compression is not supported")]
    UnsupportedPackage,
}

/// A raw native input or the database captured from an inert CAB package.
/// Read/Seek expose database bytes, never the outer package. Package signatures
/// are unverified; this grants no execution trust or writable-session capability.
pub struct AccessInput<R, S = Cursor<Vec<u8>>> {
    inner: Input<R, S>,
}
enum Input<R, S> {
    Native(R),
    Deployment(S),
    Captured(Cursor<Vec<u8>>),
}
impl<R: Read + Seek> AccessInput<R> {
    /// Select by content, capturing one qualified package database in memory.
    /// Sources must be stable/offline. No disk extraction or active content runs.
    pub fn open(
        mut source: R,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AccessInputError> {
        budget
            .charge(Resource::Bytes, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut magic = [0; size_of::<u32>()];
        tablecraft_io::read_exact_at(&mut source, 0, &mut magic, budget, cancelled)
            .map_err(InspectError::from)?;
        let inner = if magic == *b"MSCF" {
            Input::Deployment(Cursor::new(deployment::read(
                &mut source,
                budget,
                cancelled,
            )?))
        } else {
            source
                .seek(SeekFrom::Start(0))
                .map_err(InspectError::from)?;
            Input::Native(source)
        };
        Ok(Self { inner })
    }
}

impl<R, S> AccessInput<R, S> {
    /// Whether this source came from a CAB deployment package. Its signature
    /// has not been verified, regardless of any signature bytes present.
    pub fn is_deployment(&self) -> bool {
        matches!(self.inner, Input::Deployment(_) | Input::Captured(_))
    }
}
impl<R: Read, S: Read> Read for AccessInput<R, S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            Input::Native(source) => source.read(bytes),
            Input::Deployment(source) => source.read(bytes),
            Input::Captured(source) => source.read(bytes),
        }
    }
}
impl<R: Seek, S: Seek> Seek for AccessInput<R, S> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        match &mut self.inner {
            Input::Native(source) => source.seek(from),
            Input::Deployment(source) => source.seek(from),
            Input::Captured(source) => source.seek(from),
        }
    }
}
impl<R: Read + Seek, S: Read + Seek> NativeReadSource for AccessInput<R, S> {}

impl<R: Read + Seek, S: Read + Write + Seek> AccessInput<R, S> {
    /// Select native bytes, the existing small CAB capture, or a host-owned spool.
    ///
    /// Streamed geometry qualification selects the existing in-memory path only
    /// when both physical package and decoded database fit its 64 MiB ceilings.
    /// Larger qualified inputs use the lazy spool factory under the same complete
    /// extraction, cleanup and budget contracts as [`Self::open_spooled`]. No
    /// failed extraction is retried or silently switched to another strategy.
    /// The factory is not called for raw native inputs or small captures.
    pub fn open_with_spool<F>(
        mut source: R,
        make_spool: F,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AccessInputError>
    where
        F: FnOnce() -> Result<S, InspectError>,
    {
        budget
            .charge(Resource::Bytes, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut magic = [0; size_of::<u32>()];
        tablecraft_io::read_exact_at(&mut source, 0, &mut magic, budget, cancelled)
            .map_err(InspectError::from)?;
        let inner = if magic == *b"MSCF" {
            match deployment::read_with_spool(&mut source, make_spool, budget, cancelled)? {
                deployment::Selected::Captured(bytes) => Input::Captured(Cursor::new(bytes)),
                deployment::Selected::Spooled(storage) => Input::Deployment(storage),
            }
        } else {
            tablecraft_io::seek_absolute(&mut source, 0, budget, cancelled)
                .map_err(InspectError::from)?;
            Input::Native(source)
        };
        Ok(Self { inner })
    }

    /// Select native bytes or completely extract a qualified CAB into host storage.
    ///
    /// The lazy factory runs only for a qualified deployment input. It must return
    /// fresh empty seekable storage; a nonempty spool is rejected without writes.
    /// Ownership is retained only on success and dropped on every error. Hosts
    /// own creation and cleanup (for example, an unnamed temporary file), source
    /// stability and I/O timeouts. No path is selected from package content.
    ///
    /// Native bytes are cumulative work, not a retained-memory estimate: metadata,
    /// decoder allocation, requested source reads, decoded chunks and spool writes
    /// are charged. Short source reads can conservatively charge bytes again.
    ///
    /// CAB checksums and exact decoded EOF are checked before publication. Package
    /// signatures remain unverified. This read-only source does not authorize
    /// native writes or raise the separate whole-image encryption rewrite ceiling.
    pub fn open_spooled<F>(
        mut source: R,
        make_spool: F,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AccessInputError>
    where
        F: FnOnce() -> Result<S, InspectError>,
    {
        budget
            .charge(Resource::Bytes, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut magic = [0; size_of::<u32>()];
        tablecraft_io::read_exact_at(&mut source, 0, &mut magic, budget, cancelled)
            .map_err(InspectError::from)?;
        let inner = if magic == *b"MSCF" {
            Input::Deployment(deployment::read_spooled(
                &mut source,
                make_spool,
                budget,
                cancelled,
            )?)
        } else {
            tablecraft_io::seek_absolute(&mut source, 0, budget, cancelled)
                .map_err(InspectError::from)?;
            Input::Native(source)
        };
        Ok(Self { inner })
    }
}
