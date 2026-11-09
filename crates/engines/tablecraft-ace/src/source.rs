//! Explicit native-page decoding, separate from raw byte I/O and preservation.
mod agile;
mod encrypted;
mod long_values;
use crate::{Header, InspectError};
pub use agile::AgileSource;
pub use encrypted::EncryptedSource;
pub use long_values::{LongTextReadLimits, LongValueReadLimits, LongValueReadSource};
use std::io::{Read, Seek};
use tablecraft_core::limits::ResourceBudget;

/// A stable/offline raw byte source with an explicit native-page interpretation.
///
/// Read/Seek always expose original bytes. Only native readers call the page hook;
/// whole-image preservation must never silently capture decrypted bytes. Plain
/// implementations use these defaults. File, Cursor, SharedFile and offline read
/// transactions already implement this trait. Custom raw sources can implement it
/// with an empty impl. Hosts own source stability and I/O timeouts.
pub trait NativeReadSource: Read + Seek {
    /// Per-value storage limits, separate from cumulative resource budgets.
    fn long_value_limits(&self) -> LongValueReadLimits {
        LongValueReadLimits::default()
    }

    /// Decoded Memo UTF-8 limit, separate from encoded storage and resource pools.
    fn long_text_limits(&self) -> LongTextReadLimits {
        LongTextReadLimits::default()
    }

    /// Attach qualified read context to this source's original header.
    /// Plain sources retain ordinary encoded-page/profile gates.
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        Ok(header)
    }
    /// Interpret an already-read native page under the supplied cumulative budget.
    /// Plain sources leave it unchanged. This must not change the underlying bytes.
    /// Native readers recheck cancellation after a successful return, before
    /// interpreting or retaining the local decoded buffer.
    fn decode_native_page(
        &mut self,
        _number: u32,
        _page: &mut [u8],
        _budget: &mut ResourceBudget,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        Ok(())
    }
}

impl NativeReadSource for std::fs::File {}
impl<T: AsRef<[u8]>> NativeReadSource for std::io::Cursor<T> {}
impl NativeReadSource for tablecraft_io::SharedFile {}
impl NativeReadSource for crate::OfflineReadTransaction {}
impl<S: NativeReadSource + ?Sized> NativeReadSource for &mut S {
    fn long_value_limits(&self) -> LongValueReadLimits {
        (**self).long_value_limits()
    }

    fn long_text_limits(&self) -> LongTextReadLimits {
        (**self).long_text_limits()
    }

    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        (**self).qualify_header(header)
    }
    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        (**self).decode_native_page(number, page, budget, cancelled)
    }
}
