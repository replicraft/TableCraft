//! Nonblocking OS file locks; not a database's shared-file locking protocol.
use crate::IoError;
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

/// A shared OS lock held by an owned, read-only file handle until it is dropped.
/// No raw/clone handle or manual unlock is exposed. Locks coordinate cooperating
/// handles; filesystem/platform semantics can permit unlocked writes or renames.
pub struct SharedFile(File);
impl SharedFile {
    /// Open an existing regular file read-only and immediately try a shared lock.
    /// Contention returns LockBusy; there is no waiting, retry or lock-file creation.
    pub fn open(path: &Path) -> Result<Self, IoError> {
        let file = File::open(path)?;
        regular(&file)?;
        file.try_lock_shared().map_err(lock_error)?;
        Ok(Self(file))
    }
    /// Current handle metadata length; this does not establish content stability.
    pub fn length(&self) -> Result<u64, IoError> {
        Ok(self.0.metadata()?.len())
    }
    /// Explicitly release the lock, reporting unlock failure. Drop still closes
    /// the handle and attempts best-effort release if this fails.
    pub fn close(self) -> Result<(), IoError> {
        self.0.unlock().map_err(Into::into)
    }
}
impl Drop for SharedFile {
    fn drop(&mut self) {
        // Explicit unlock prevents a concurrently forked child's inherited
        // descriptor from extending this guard's lock lifetime until exec.
        // Drop cannot report errors; File close remains the fallback. Callers
        // requiring a reported release use close().
        let _ = self.0.unlock();
    }
}
impl Read for SharedFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.read(bytes)
    }
}
impl Seek for SharedFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}

/// An owned read/write handle holding an exclusive OS lock until dropped.
/// This low-level type supplies no database validation or native lock protocol.
pub struct ExclusiveFile(File);
impl ExclusiveFile {
    /// Open an existing regular file without truncation and try an exclusive lock.
    /// The caller owns file creation, durable namespace setup and path identity.
    pub fn open(path: &Path) -> Result<Self, IoError> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        regular(&file)?;
        file.try_lock().map_err(lock_error)?;
        Ok(Self(file))
    }
    /// Current handle metadata length.
    pub fn length(&self) -> Result<u64, IoError> {
        Ok(self.0.metadata()?.len())
    }
    /// Request persistence of this file's contents and metadata, not its directory.
    pub fn sync_all(&mut self) -> io::Result<()> {
        self.0.sync_all()
    }
    /// Explicitly release the lock, reporting unlock failure before handle close.
    pub fn close(self) -> Result<(), IoError> {
        self.0.unlock().map_err(Into::into)
    }
}
impl Drop for ExclusiveFile {
    fn drop(&mut self) {
        // See SharedFile's destructor and the explicit close() error path.
        let _ = self.0.unlock();
    }
}
impl Read for ExclusiveFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.read(bytes)
    }
}
impl Write for ExclusiveFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl Seek for ExclusiveFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}
fn regular(file: &File) -> Result<(), IoError> {
    if !file.metadata()?.is_file() {
        return Err(IoError::Io(io::ErrorKind::InvalidInput));
    }
    Ok(())
}
fn lock_error(error: TryLockError) -> IoError {
    match error {
        TryLockError::WouldBlock => IoError::LockBusy,
        TryLockError::Error(error) => error.into(),
    }
}
