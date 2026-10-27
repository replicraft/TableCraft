//! Publish only a complete candidate to a newly created, caller-selected path.
use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
};

pub(super) fn publish(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    publish_with(path, bytes, |output, bytes| {
        output.write_all(bytes)?;
        output.sync_all()
    })
}

fn publish_with(
    path: &Path,
    bytes: &[u8],
    write_and_sync: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
) -> Result<(), Box<dyn Error>> {
    let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = write_and_sync(&mut output, bytes) {
        drop(output);
        if let Err(cleanup) = fs::remove_file(path) {
            return Err(format!(
                "Candidate write failed ({error}); incomplete-output cleanup failed ({cleanup})"
            )
            .into());
        }
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "output/tests.rs"]
mod tests;
