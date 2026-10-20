use super::NativeReadSource;
use crate::{AgilePageDecoder, Header, InspectError, inspect_header, read_agile_encryption};
use std::io::{self, Read, Seek, SeekFrom};
use tablecraft_core::limits::ResourceBudget;

/// Password-verified native page source. Raw Read/Seek retain original ciphertext.
///
/// No Write, Debug, Clone, secret serialization or plaintext image interface is
/// supplied. Library table readers explicitly decode pages under their own budgets.
/// The source must remain stable/offline for its lifetime; this does not acquire
/// shared-file coordination or establish ciphertext authenticity.
pub struct AgileSource<R> {
    source: R,
    decoder: AgilePageDecoder,
    header: Header,
}
impl<R: Read + Seek> AgileSource<R> {
    /// Verify explicit credentials once and bind the decoder to the owned source.
    /// Metadata, password derivation and subsequent native reads must share the
    /// host's cumulative budget. This source caches no plaintext pages; native
    /// cursors and snapshots retain decoded data under their normal lifetimes.
    pub fn open(
        mut source: R,
        password: Option<&str>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let header = inspect_header(&mut source, budget, cancelled)?;
        let encrypted = read_agile_encryption(&mut source, budget, cancelled)?;
        let decoder = encrypted.unlock(password, budget, cancelled)?;
        // Selecting geometry remains separate from successful authentication.
        header.decoded().read_profile()?;
        Ok(Self {
            source,
            decoder,
            header,
        })
    }
    /// Original raw header, without a decoded-layout read context.
    pub const fn raw_header(&self) -> Header {
        self.header
    }
    /// Release the decoder and return the unchanged raw source.
    pub fn into_inner(self) -> R {
        self.source
    }
}
impl<R: Read> Read for AgileSource<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.source.read(bytes)
    }
}
impl<R: Seek> Seek for AgileSource<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.source.seek(from)
    }
}
impl<R: Read + Seek> NativeReadSource for AgileSource<R> {
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
