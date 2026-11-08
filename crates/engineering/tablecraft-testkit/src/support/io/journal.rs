//! In-memory durable-storage protocol observations, without filesystem guarantees.
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_io::{IoError, journal::DurableStorage};

/// Byte storage with write-call accounting and explicit no-op test barriers.
/// This models the journal protocol, never power-loss or filesystem durability.
pub struct MemoryStorage {
    /// Complete bytes and cursor position, exposed for independent corruption controls.
    pub bytes: Cursor<Vec<u8>>,
    /// Number of write calls, including writes that leave image bytes unchanged.
    pub writes: usize,
}

impl MemoryStorage {
    /// Start with caller-owned bytes and no observed write calls.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            writes: 0,
        }
    }
}

impl Read for MemoryStorage {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}

impl Seek for MemoryStorage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl Write for MemoryStorage {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        self.bytes.write(input)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush must not replace the test storage barrier")
    }
}

impl DurableStorage for MemoryStorage {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        Ok(())
    }
}
