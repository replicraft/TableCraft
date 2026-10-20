use super::NativeReadSource;
use crate::{Header, InspectError, encryption::PageDecoder, inspect_header};
use std::io::{self, Read, Seek, SeekFrom};
use tablecraft_core::limits::ResourceBudget;

/// Explicit decoded-page source for qualified native Jet/Office encryption.
/// Raw Read/Seek return original ciphertext. No Write, Debug, Clone, plaintext
/// image capture or key export is provided. Use only a stable/offline source.
pub struct EncryptedSource<R> {
    source: R,
    header: Header,
    decoder: PageDecoder,
}

impl<R: Read + Seek> EncryptedSource<R> {
    /// Select the declared codec and verify its credentials under one allowance.
    /// Jet file encoding does not authenticate a database password; its key is
    /// stored in page zero. Office schemes require explicit password input.
    pub fn open(
        mut source: R,
        password: Option<&str>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let header = inspect_header(&mut source, budget, cancelled)?;
        header.decoded().read_profile()?;
        let decoder = PageDecoder::open(&mut source, password, budget, cancelled)?;
        Ok(Self {
            source,
            header,
            decoder,
        })
    }
    /// Original header, without a decoded read context.
    pub const fn raw_header(&self) -> Header {
        self.header
    }
    /// Drop the codec and recover the unchanged raw source.
    pub fn into_inner(self) -> R {
        self.source
    }
}

impl<R: Read> Read for EncryptedSource<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.source.read(bytes)
    }
}
impl<R: Seek> Seek for EncryptedSource<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.source.seek(from)
    }
}
impl<R: Read + Seek> NativeReadSource for EncryptedSource<R> {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        if header != self.header {
            return Err(InspectError::Malformed);
        }
        Ok(header.decoded())
    }
    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        self.decoder.decode_page(number, page, budget, cancelled)
    }
}
