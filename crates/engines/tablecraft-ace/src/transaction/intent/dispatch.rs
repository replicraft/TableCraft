//! Closed native validators selected by exact request-record version/profile.
use super::{NativeInt32EditIntent, NativeIntentError};
use crate::{Int32Update, header::check};
use tablecraft_core::limits::ResourceBudget;

const JET4_MAGIC: &[u8; 8] = b"TCNI0001";
const ACE_MAGIC: &[u8; 8] = b"TCNI0002";
const MAGIC_PREFIX: &[u8; 4] = b"TCNI";
const ACE_PROFILE: u16 = 2;

/// A versioned validation route, never a serialized native permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeUpdateProfile {
    /// Unchanged ordinary Jet4 Int32 request record and planner.
    Jet4Int32,
    /// Positive source-derived pure BeforeChange ACE16 Int32 UPDATE.
    Ace16BeforeChangeInt32,
}

/// Immutable original user intent and BEFORE binding for one normal route.
/// Macro final rows/assignments/selectors are deliberately absent. Recovery
/// rederives its complete native plan before any write, regardless of this tag.
pub struct NativeUpdateIntent {
    profile: NativeUpdateProfile,
    original: NativeInt32EditIntent,
}

impl NativeUpdateIntent {
    /// Both versions share the same bounded original-request payload geometry.
    pub const MAX_ENCODED_BYTES: usize = NativeInt32EditIntent::MAX_ENCODED_BYTES;

    pub(super) fn from_original(
        profile: NativeUpdateProfile,
        original: NativeInt32EditIntent,
    ) -> Self {
        Self { profile, original }
    }

    /// Exact record-selected validator, not source eligibility or event coverage.
    pub const fn profile(&self) -> NativeUpdateProfile {
        self.profile
    }

    /// Original source-selected table spelling, without SQL or file selectors.
    pub fn table(&self) -> &str {
        self.original.table()
    }

    /// Original user-request column; macro-only targets are reconstructed later.
    pub fn column(&self) -> &str {
        self.original.column()
    }

    /// Original written requests, including every supplied equal assignment.
    pub fn updates(&self) -> &[Int32Update] {
        self.original.updates()
    }

    /// Exact original source byte length.
    pub const fn before_length(&self) -> u64 {
        self.original.before_length()
    }

    /// Compare complete original-source digest, without granting replay permission.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        self.original.verify_before(before, budget, cancelled)
    }

    /// Encode the selected closed record. Jet4 emits byte-identical TCNI0001.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        match self.profile {
            NativeUpdateProfile::Jet4Int32 => self.original.encode(budget, cancelled),
            NativeUpdateProfile::Ace16BeforeChangeInt32 => {
                self.original
                    .encode_profile(ACE_MAGIC, ACE_PROFILE, budget, cancelled)
            }
        }
    }

    /// Decode exactly one version/profile; no catch/retry or cross-route fallback.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let (profile, original) = match bytes.get(..JET4_MAGIC.len()) {
            Some(magic) if magic == JET4_MAGIC => (
                NativeUpdateProfile::Jet4Int32,
                NativeInt32EditIntent::decode(bytes, budget, cancelled)?,
            ),
            Some(magic) if magic == ACE_MAGIC => (
                NativeUpdateProfile::Ace16BeforeChangeInt32,
                NativeInt32EditIntent::decode_profile(
                    bytes,
                    ACE_MAGIC,
                    ACE_PROFILE,
                    budget,
                    cancelled,
                )?,
            ),
            _ => {
                check(budget, cancelled)?;
                return Err(if bytes.starts_with(MAGIC_PREFIX) {
                    NativeIntentError::UnsupportedVersion
                } else {
                    NativeIntentError::InvalidRecord
                });
            }
        };
        Ok(Self { profile, original })
    }

    pub(in crate::transaction) fn into_original(
        self,
    ) -> (NativeUpdateProfile, NativeInt32EditIntent) {
        (self.profile, self.original)
    }
}
