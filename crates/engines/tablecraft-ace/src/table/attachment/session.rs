//! Reuse one codec workspace within one caller-owned cumulative budget.
use super::{AttachmentDecodeLimits, DecodedAttachment, decode};
use crate::{InspectError, header::check};
use flate2::Decompress;
use tablecraft_core::limits::ResourceBudget;

/// Explicit attachment decoding session with one lazily allocated zlib workspace.
/// Every compressed stream resets the codec, including after errors. The borrowed
/// budget/cancellation policy stays fixed; outputs, work and copies remain charged
/// per file. This retains no encoded input, decoded files or cross-file dictionary.
pub struct AttachmentDecoder<'a> {
    inflater: Option<Decompress>,
    budget: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
    limits: AttachmentDecodeLimits,
}

impl<'a> AttachmentDecoder<'a> {
    /// Start a session without allocating a codec or decoding any content.
    pub fn new(budget: &'a mut ResourceBudget, cancelled: &'a dyn Fn() -> bool) -> Self {
        Self::new_with_limits(budget, cancelled, AttachmentDecodeLimits::default())
    }

    /// Start a session with an explicit per-file allowance and the same fixed pool.
    pub fn new_with_limits(
        budget: &'a mut ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
        limits: AttachmentDecodeLimits,
    ) -> Self {
        Self {
            inflater: None,
            budget,
            cancelled,
            limits,
        }
    }

    /// Decode one independent native wrapper under this session's original pool.
    /// Failure publishes no decoded file and never retries the input.
    pub fn decode(&mut self, encoded: &[u8]) -> Result<DecodedAttachment, InspectError> {
        decode::using_inflater(
            encoded,
            &mut self.inflater,
            self.limits,
            self.budget,
            self.cancelled,
        )
    }

    /// Charge additional consumer-owned copies/formatting against the same pool.
    /// Callers must preserve the original cumulative budget rather than replacing it.
    pub fn budget_mut(&mut self) -> &mut ResourceBudget {
        self.budget
    }

    pub(crate) fn decode_optional(
        &mut self,
        encoded: Option<&[u8]>,
    ) -> Result<Option<DecodedAttachment>, InspectError> {
        check(self.budget, self.cancelled)?;
        encoded.map(|bytes| self.decode(bytes)).transpose()
    }
}
