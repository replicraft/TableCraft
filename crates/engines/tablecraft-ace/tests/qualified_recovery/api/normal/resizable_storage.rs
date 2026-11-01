//! The old fixed-only fixture remains fixed; this wrapper issues resize explicitly.
use super::Storage;
use std::io::{self, Read, Seek, SeekFrom, Write};
use tablecraft_io::{
    IoError,
    journal::{DurableStorage, ResizableDurableStorage},
};

pub(super) struct Target {
    pub storage: Storage,
    pub resizes: usize,
}

impl Target {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            storage: Storage::new(bytes),
            resizes: 0,
        }
    }
}

impl Read for Target {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.storage.read(output)
    }
}

impl Seek for Target {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.storage.seek(position)
    }
}

impl Write for Target {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.storage.write(input)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.storage.flush()
    }
}

impl DurableStorage for Target {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.storage.storage_len()
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.storage.sync_storage()
    }
}

impl ResizableDurableStorage for Target {
    fn resize_storage(&mut self, length: u64) -> Result<(), IoError> {
        self.resizes += 1;
        let length =
            usize::try_from(length).map_err(|_| IoError::Io(io::ErrorKind::InvalidInput))?;
        self.storage.bytes.get_mut().resize(length, 0);
        Ok(())
    }
}
