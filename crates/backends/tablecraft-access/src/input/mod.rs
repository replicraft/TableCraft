//! Read-only native input and bounded deployment-container selection.
mod deployment;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
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
pub struct AccessInput<R> {
    inner: Input<R>,
}
enum Input<R> {
    Native(R),
    Deployment(Cursor<Vec<u8>>),
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

    /// Whether this source came from a CAB deployment package. Its signature
    /// has not been verified, regardless of any signature bytes present.
    pub fn is_deployment(&self) -> bool {
        matches!(self.inner, Input::Deployment(_))
    }
}
impl<R: Read> Read for AccessInput<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            Input::Native(source) => source.read(bytes),
            Input::Deployment(source) => source.read(bytes),
        }
    }
}
impl<R: Seek> Seek for AccessInput<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        match &mut self.inner {
            Input::Native(source) => source.seek(from),
            Input::Deployment(source) => source.seek(from),
        }
    }
}
impl<R: Read + Seek> NativeReadSource for AccessInput<R> {}
