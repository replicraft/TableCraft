//! Open ordinary input files without waiting for a Unix FIFO peer.
#[cfg(unix)]
use rustix::fs::{Mode, OFlags};
use std::{fs::File, io, path::Path};

/// Open an existing regular file read-only, preserving ordinary host IO errors.
/// Unix opens with NONBLOCK before checking the actual descriptor's file kind,
/// so a FIFO is rejected without waiting for a peer. Symlinks retain ordinary
/// File::open behavior. This does not lock the file or establish stable content.
/// Other platforms retain their standard read-only open before the kind check;
/// callers still own timeout policy for individual host callbacks.
pub fn open_regular_readonly(path: &Path) -> io::Result<File> {
    let _span = tracing::debug_span!("io.open_regular_input").entered();
    #[cfg(unix)]
    let file = {
        let descriptor = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        File::from(descriptor)
    };
    #[cfg(not(unix))]
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(file)
}

#[cfg(test)]
mod tests;
