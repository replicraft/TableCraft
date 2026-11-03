//! Explicitly mutable original-byte source for adversarial read-binding tests.
use std::{
    cell::RefCell,
    io::{self, Read, Seek, SeekFrom},
    rc::Rc,
};

/// Tests may change the shared image between read operations. This deliberately
/// violates the normal stable-source promise and supplies no coherent-read claim.
pub struct MutableSource {
    bytes: Rc<RefCell<Vec<u8>>>,
    position: u64,
}

impl MutableSource {
    /// Own a fresh independently supplied raw image at position zero.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Rc::new(RefCell::new(bytes)),
            position: 0,
        }
    }

    /// Retain a controlled mutation handle for changes between read calls.
    pub fn image(&self) -> Rc<RefCell<Vec<u8>>> {
        Rc::clone(&self.bytes)
    }
}

impl tablecraft_ace::NativeReadSource for MutableSource {}

impl Read for MutableSource {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let bytes = self
            .bytes
            .try_borrow()
            .map_err(|_| io::Error::other("test source is borrowed"))?;
        let start = usize::try_from(self.position)
            .unwrap_or(usize::MAX)
            .min(bytes.len());
        let count = output.len().min(bytes.len() - start);
        output[..count].copy_from_slice(&bytes[start..start + count]);
        self.position = self
            .position
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("test source position overflow"))?;
        Ok(count)
    }
}

impl Seek for MutableSource {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let next = match from {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => {
                let bytes = self
                    .bytes
                    .try_borrow()
                    .map_err(|_| io::Error::other("test source is borrowed"))?;
                bytes.len() as i128 + i128::from(offset)
            }
        };
        self.position = u64::try_from(next)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid test source seek"))?;
        Ok(self.position)
    }
}
