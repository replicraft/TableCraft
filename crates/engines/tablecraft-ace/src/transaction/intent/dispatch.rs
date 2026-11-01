//! Closed native validators selected by exact request-record version/profile.
use super::{
    NativeFittedEditIntent, NativeFixedEditIntent, NativeInt32EditIntent, NativeIntentError,
};
use crate::header::check;
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
    /// Positive source-derived pure BeforeChange ACE16 fixed-group UPDATE.
    Ace16BeforeChangeFixed,
    /// Ordinary ACE16 fixed groups with complete content closure rederived natively.
    Ace16OrdinaryFixed,
    /// Ordinary ACE16 fitting variable requests with optional fixed groups.
    Ace16OrdinaryFitted,
    /// Explicit ordinary growth within existing owned DATA capacity; no EOF growth.
    Ace16OrdinaryOwnedGrowth,
}

/// Immutable original user intent and BEFORE binding for one normal route.
/// Macro final rows/assignments/selectors are deliberately absent. Recovery
/// rederives its complete native plan before any write, regardless of this tag.
pub struct NativeUpdateIntent {
    profile: NativeUpdateProfile,
    body: NativeUpdateBody,
}

/// Exact original request family; no missing body is replaced with empty data.
pub enum NativeUpdateBody {
    /// Original single-column Int32 requests for TCNI0001 or TCNI0002.
    Int32(NativeInt32EditIntent),
    /// Original ordered fixed column groups for TCNI0003 or TCNI0004.
    Fixed(NativeFixedEditIntent),
    /// Ordered original fixed/variable requests for TCNI0005.
    Fitted(NativeFittedEditIntent),
    /// Original fixed/variable groups for explicit TCNI0006 owned-page growth.
    OwnedGrowth(NativeFittedEditIntent),
}

impl NativeUpdateIntent {
    /// Maximum of the implemented closed data records, for bounded host capture.
    pub const MAX_ENCODED_BYTES: usize = if NativeFittedEditIntent::MAX_ENCODED_BYTES
        > NativeFixedEditIntent::MAX_ENCODED_BYTES
        && NativeFittedEditIntent::MAX_ENCODED_BYTES > NativeInt32EditIntent::MAX_ENCODED_BYTES
    {
        NativeFittedEditIntent::MAX_ENCODED_BYTES
    } else if NativeFixedEditIntent::MAX_ENCODED_BYTES > NativeInt32EditIntent::MAX_ENCODED_BYTES {
        NativeFixedEditIntent::MAX_ENCODED_BYTES
    } else {
        NativeInt32EditIntent::MAX_ENCODED_BYTES
    };

    pub(super) fn from_original(
        profile: NativeUpdateProfile,
        original: NativeInt32EditIntent,
    ) -> Self {
        Self {
            profile,
            body: NativeUpdateBody::Int32(original),
        }
    }

    /// Exact record-selected validator, not source eligibility or event coverage.
    pub const fn profile(&self) -> NativeUpdateProfile {
        self.profile
    }

    pub(super) fn from_fixed(original: NativeFixedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16BeforeChangeFixed,
            body: NativeUpdateBody::Fixed(original),
        }
    }

    pub(super) fn from_ordinary_fixed(original: NativeFixedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16OrdinaryFixed,
            body: NativeUpdateBody::Fixed(original),
        }
    }

    pub(super) fn from_fitted(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16OrdinaryFitted,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_owned_growth(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16OrdinaryOwnedGrowth,
            body: NativeUpdateBody::OwnedGrowth(original),
        }
    }

    /// Closed original body, requiring consumers to inspect its actual family.
    pub fn body(&self) -> &NativeUpdateBody {
        &self.body
    }

    /// Int32 original data, absent for a fixed-group request.
    pub fn int32(&self) -> Option<&NativeInt32EditIntent> {
        match &self.body {
            NativeUpdateBody::Int32(original) => Some(original),
            NativeUpdateBody::Fixed(_)
            | NativeUpdateBody::Fitted(_)
            | NativeUpdateBody::OwnedGrowth(_) => None,
        }
    }

    /// Fixed original data, absent for an Int32 request.
    pub fn fixed(&self) -> Option<&NativeFixedEditIntent> {
        match &self.body {
            NativeUpdateBody::Fixed(original) => Some(original),
            NativeUpdateBody::Int32(_)
            | NativeUpdateBody::Fitted(_)
            | NativeUpdateBody::OwnedGrowth(_) => None,
        }
    }

    /// Original fitted data, absent for an older fixed or Int32 record.
    pub fn fitted(&self) -> Option<&NativeFittedEditIntent> {
        match &self.body {
            NativeUpdateBody::Fitted(original) => Some(original),
            NativeUpdateBody::Int32(_)
            | NativeUpdateBody::Fixed(_)
            | NativeUpdateBody::OwnedGrowth(_) => None,
        }
    }

    /// Explicit owned-page growth originals; absent for the unchanged fitting route.
    pub fn owned_growth(&self) -> Option<&NativeFittedEditIntent> {
        match &self.body {
            NativeUpdateBody::OwnedGrowth(original) => Some(original),
            NativeUpdateBody::Int32(_)
            | NativeUpdateBody::Fixed(_)
            | NativeUpdateBody::Fitted(_) => None,
        }
    }

    /// Original table spelling, without SQL or file selectors.
    pub fn table(&self) -> &str {
        match &self.body {
            NativeUpdateBody::Int32(original) => original.table(),
            NativeUpdateBody::Fixed(original) => original.table(),
            NativeUpdateBody::Fitted(original) | NativeUpdateBody::OwnedGrowth(original) => {
                original.table()
            }
        }
    }

    /// Exact original source byte length.
    pub fn before_length(&self) -> u64 {
        match &self.body {
            NativeUpdateBody::Int32(original) => original.before_length(),
            NativeUpdateBody::Fixed(original) => original.before_length(),
            NativeUpdateBody::Fitted(original) | NativeUpdateBody::OwnedGrowth(original) => {
                original.before_length()
            }
        }
    }

    /// Compare complete original-source digest, without granting replay permission.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        match &self.body {
            NativeUpdateBody::Int32(original) => original.verify_before(before, budget, cancelled),
            NativeUpdateBody::Fixed(original) => original.verify_before(before, budget, cancelled),
            NativeUpdateBody::Fitted(original) | NativeUpdateBody::OwnedGrowth(original) => {
                original.verify_before(before, budget, cancelled)
            }
        }
    }

    /// Encode the selected closed record. Jet4 emits byte-identical TCNI0001.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        match (&self.body, self.profile) {
            (NativeUpdateBody::Int32(original), NativeUpdateProfile::Jet4Int32) => {
                original.encode(budget, cancelled)
            }
            (NativeUpdateBody::Int32(original), NativeUpdateProfile::Ace16BeforeChangeInt32) => {
                original.encode_profile(ACE_MAGIC, ACE_PROFILE, budget, cancelled)
            }
            (NativeUpdateBody::Fixed(original), NativeUpdateProfile::Ace16BeforeChangeFixed) => {
                original.encode(budget, cancelled)
            }
            (NativeUpdateBody::Fixed(original), NativeUpdateProfile::Ace16OrdinaryFixed) => {
                original.encode_ordinary(budget, cancelled)
            }
            (NativeUpdateBody::Fitted(original), NativeUpdateProfile::Ace16OrdinaryFitted) => {
                original.encode(budget, cancelled)
            }
            (
                NativeUpdateBody::OwnedGrowth(original),
                NativeUpdateProfile::Ace16OrdinaryOwnedGrowth,
            ) => original.encode_profile(super::fitted::WireProfile::OwnedOnly, budget, cancelled),
            _ => Err(NativeIntentError::UnsupportedProfile),
        }
    }

    /// Decode exactly one version/profile; no catch/retry or cross-route fallback.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        if bytes.get(..super::fitted::OWNED_GROWTH_MAGIC.len())
            == Some(super::fitted::OWNED_GROWTH_MAGIC.as_slice())
        {
            return Ok(Self::from_owned_growth(
                NativeFittedEditIntent::decode_profile(
                    bytes,
                    super::fitted::WireProfile::OwnedOnly,
                    budget,
                    cancelled,
                )?,
            ));
        }
        if bytes.get(..super::fitted::MAGIC.len()) == Some(super::fitted::MAGIC.as_slice()) {
            return Ok(Self::from_fitted(NativeFittedEditIntent::decode(
                bytes, budget, cancelled,
            )?));
        }
        if bytes.get(..NativeFixedEditIntent::ORDINARY_MAGIC.len())
            == Some(NativeFixedEditIntent::ORDINARY_MAGIC.as_slice())
        {
            return Ok(Self::from_ordinary_fixed(
                NativeFixedEditIntent::decode_ordinary(bytes, budget, cancelled)?,
            ));
        }
        if bytes.get(..8) == Some(super::fixed::MAGIC.as_slice()) {
            return Ok(Self::from_fixed(NativeFixedEditIntent::decode(
                bytes, budget, cancelled,
            )?));
        }
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
        Ok(Self {
            profile,
            body: NativeUpdateBody::Int32(original),
        })
    }

    pub(in crate::transaction) fn into_body(self) -> (NativeUpdateProfile, NativeUpdateBody) {
        (self.profile, self.body)
    }
}
