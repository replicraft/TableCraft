mod limits;

use super::NativeReadSource;
use crate::{Header, InspectError};
pub use limits::LongValueReadLimits;
use std::io::{self, Read, Seek, SeekFrom};
use tablecraft_core::limits::ResourceBudget;

/// Explicit per-value read policy around an already qualified native source.
///
/// Wrap outside encryption/container sources. Raw Read/Seek still expose original
/// bytes; native header/page hooks delegate to the wrapped source. Cumulative
/// budgets, decoded text/file limits and source stability requirements are unchanged.
pub struct LongValueReadSource<R> {
    source: R,
    limits: LongValueReadLimits,
}

impl<R> LongValueReadSource<R> {
    /// Attach a validated policy without reading or modifying the source.
    pub fn new(source: R, limits: LongValueReadLimits) -> Self {
        Self { source, limits }
    }

    /// Recover the original source, including its current cursor position.
    pub fn into_inner(self) -> R {
        self.source
    }
}

impl<R: Read> Read for LongValueReadSource<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.source.read(bytes)
    }
}

impl<R: Seek> Seek for LongValueReadSource<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.source.seek(position)
    }
}

impl<R: NativeReadSource> NativeReadSource for LongValueReadSource<R> {
    fn long_value_limits(&self) -> LongValueReadLimits {
        self.limits
    }

    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        self.source.qualify_header(header)
    }

    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        self.source
            .decode_native_page(number, page, budget, cancelled)
    }
}
