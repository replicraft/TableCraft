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
    /// A Boolean absent from the complete original record, requalified before materialization.
    Ace16RetainedBooleanMaterialization,
    /// Exact canonical GUID and Decimal originals in complete retained records.
    Ace16RetainedWideValueRows,
    /// Exact nullable Byte/Int16/Int64/Single/Double/legacy DateTime retained cells.
    Ace16RetainedFixedScalarRows,
    /// Present complete retained Boolean cells with exact original bit values.
    Ace16RetainedBooleanRows,
    /// Complete retained Currency cells with exact scaled original witnesses.
    Ace16RetainedCurrencyRows,
    /// Retained historical opaque tails with exact original LONG cell witnesses.
    Ace16RetainedOpaqueRows,
    /// Retained surroundings with exact current Number/Text request witnesses.
    Ace16RetainedRows,
    /// Current sparse native metadata and checked original Number/Text carriers.
    Ace16CurrentRows,
    /// One Short Text field updated in a positively qualified historical row.
    Ace16HistoricalShortTextUpdates,
    /// One absent Number materialized through a qualified relocated historical root.
    Ace16HistoricalForwardedNumberMaterialization,
    /// One absent Number in a qualified historical row with retained variable fields.
    Ace16HistoricalMixedNumberMaterialization,
    /// One absent nullable Number field materialized within qualified owned DATA capacity.
    Ace16HistoricalNumberMaterialization,
    /// Existing unindexed Int32 fields in positively qualified historical records.
    Ace16HistoricalPresentFixed,
    /// Unchanged ordinary Jet4 Int32 request record and planner.
    Jet4Int32,
    /// Positive source-derived pure BeforeChange ACE16 Int32 UPDATE.
    Ace16BeforeChangeInt32,
    /// Positive source-derived pure BeforeChange ACE16 fixed-group UPDATE.
    Ace16BeforeChangeFixed,
    /// Ordinary ACE16 fixed groups with complete content closure rederived natively.
    Ace16OrdinaryFixed,
    /// Ordinary exact IEEE fixed groups; legacy fixed record tags stay closed.
    Ace16OrdinaryIeeeFixed,
    /// Cumulative ordinary fixed cells, including native Byte and Int16.
    Ace16OrdinaryScalarFixed,
    /// Cumulative ordinary fixed requests with canonical sixteen-byte GUIDs.
    Ace16OrdinaryGuidFixed,
    /// Ordinary ACE16 fitting variable requests with optional fixed groups.
    Ace16OrdinaryFitted,
    /// Explicit ordinary growth within existing owned DATA capacity; no EOF growth.
    Ace16OrdinaryOwnedGrowth,
    /// Original ordinary requests requiring one supported EOF DATA page.
    Ace16OrdinaryEofAllocation,
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
    /// Original ordered fixed groups for closed TCNI0003/4/8/9/10/11/12/13/14 records.
    Fixed(NativeFixedEditIntent),
    /// Ordered original variable/fixed requests for TCNI0005/15/16/17/18/19/20/21/22.
    Fitted(NativeFittedEditIntent),
    /// Original fixed/variable groups for explicit TCNI0006 owned-page growth.
    OwnedGrowth(NativeFittedEditIntent),
    /// Original fixed/variable groups for TCNI0007 one-page EOF allocation.
    EofAllocation(NativeFittedEditIntent),
}

impl NativeUpdateIntent {
    pub(super) fn from_retained_boolean_materialization(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16RetainedBooleanMaterialization,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_retained_wide_value_rows(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16RetainedWideValueRows,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_retained_fixed_scalar_rows(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16RetainedFixedScalarRows,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_retained_boolean_rows(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16RetainedBooleanRows,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_retained_currency_rows(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16RetainedCurrencyRows,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_retained_opaque_rows(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16RetainedOpaqueRows,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_retained_rows(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16RetainedRows,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    /// Actual retained TCNI17–21 u32 complete-record length representation.
    pub const MAX_RETAINED_ENCODED_BYTES: usize =
        NativeFittedEditIntent::MAX_RETAINED_ENCODED_BYTES;

    pub(super) fn from_current_rows(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16CurrentRows,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_historical_short_text_updates(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16HistoricalShortTextUpdates,
            body: NativeUpdateBody::Fitted(original),
        }
    }

    pub(super) fn from_historical_forwarded_number_materialization(
        original: NativeFixedEditIntent,
    ) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16HistoricalForwardedNumberMaterialization,
            body: NativeUpdateBody::Fixed(original),
        }
    }

    pub(super) fn from_historical_mixed_number_materialization(
        original: NativeFixedEditIntent,
    ) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16HistoricalMixedNumberMaterialization,
            body: NativeUpdateBody::Fixed(original),
        }
    }

    pub(super) fn from_historical_number_materialization(original: NativeFixedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16HistoricalNumberMaterialization,
            body: NativeUpdateBody::Fixed(original),
        }
    }

    pub(super) fn from_historical_present_fixed(original: NativeFixedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16HistoricalPresentFixed,
            body: NativeUpdateBody::Fixed(original),
        }
    }

    /// Maximum legacy record size; retained TCNI17 uses MAX_RETAINED_ENCODED_BYTES.
    pub const MAX_ENCODED_BYTES: usize = if NativeFittedEditIntent::MAX_ENCODED_BYTES
        > NativeFixedEditIntent::MAX_GUID_ENCODED_BYTES
        && NativeFittedEditIntent::MAX_ENCODED_BYTES > NativeInt32EditIntent::MAX_ENCODED_BYTES
    {
        NativeFittedEditIntent::MAX_ENCODED_BYTES
    } else if NativeFixedEditIntent::MAX_GUID_ENCODED_BYTES
        > NativeInt32EditIntent::MAX_ENCODED_BYTES
    {
        NativeFixedEditIntent::MAX_GUID_ENCODED_BYTES
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

    pub(super) fn from_ordinary_ieee_fixed(original: NativeFixedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16OrdinaryIeeeFixed,
            body: NativeUpdateBody::Fixed(original),
        }
    }

    pub(super) fn from_ordinary_scalar_fixed(original: NativeFixedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16OrdinaryScalarFixed,
            body: NativeUpdateBody::Fixed(original),
        }
    }

    pub(super) fn from_ordinary_guid_fixed(original: NativeFixedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16OrdinaryGuidFixed,
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

    pub(super) fn from_eof_allocation(original: NativeFittedEditIntent) -> Self {
        Self {
            profile: NativeUpdateProfile::Ace16OrdinaryEofAllocation,
            body: NativeUpdateBody::EofAllocation(original),
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
            | NativeUpdateBody::OwnedGrowth(_)
            | NativeUpdateBody::EofAllocation(_) => None,
        }
    }

    /// Fixed original data, absent for an Int32 request.
    pub fn fixed(&self) -> Option<&NativeFixedEditIntent> {
        match &self.body {
            NativeUpdateBody::Fixed(original) => Some(original),
            NativeUpdateBody::Int32(_)
            | NativeUpdateBody::Fitted(_)
            | NativeUpdateBody::OwnedGrowth(_)
            | NativeUpdateBody::EofAllocation(_) => None,
        }
    }

    /// Original fitted data, absent for an older fixed or Int32 record.
    pub fn fitted(&self) -> Option<&NativeFittedEditIntent> {
        match &self.body {
            NativeUpdateBody::Fitted(original) => Some(original),
            NativeUpdateBody::Int32(_)
            | NativeUpdateBody::Fixed(_)
            | NativeUpdateBody::OwnedGrowth(_)
            | NativeUpdateBody::EofAllocation(_) => None,
        }
    }

    /// Explicit owned-page growth originals; absent for the unchanged fitting route.
    pub fn owned_growth(&self) -> Option<&NativeFittedEditIntent> {
        match &self.body {
            NativeUpdateBody::OwnedGrowth(original) => Some(original),
            NativeUpdateBody::Int32(_)
            | NativeUpdateBody::Fixed(_)
            | NativeUpdateBody::Fitted(_)
            | NativeUpdateBody::EofAllocation(_) => None,
        }
    }

    /// Original allocation data; no chosen page, map claim or permission is stored.
    pub fn eof_allocation(&self) -> Option<&NativeFittedEditIntent> {
        match &self.body {
            NativeUpdateBody::EofAllocation(original) => Some(original),
            NativeUpdateBody::Int32(_)
            | NativeUpdateBody::Fixed(_)
            | NativeUpdateBody::Fitted(_)
            | NativeUpdateBody::OwnedGrowth(_) => None,
        }
    }

    /// Original table spelling, without SQL or file selectors.
    pub fn table(&self) -> &str {
        match &self.body {
            NativeUpdateBody::Int32(original) => original.table(),
            NativeUpdateBody::Fixed(original) => original.table(),
            NativeUpdateBody::Fitted(original)
            | NativeUpdateBody::OwnedGrowth(original)
            | NativeUpdateBody::EofAllocation(original) => original.table(),
        }
    }

    /// Exact original source byte length.
    pub fn before_length(&self) -> u64 {
        match &self.body {
            NativeUpdateBody::Int32(original) => original.before_length(),
            NativeUpdateBody::Fixed(original) => original.before_length(),
            NativeUpdateBody::Fitted(original)
            | NativeUpdateBody::OwnedGrowth(original)
            | NativeUpdateBody::EofAllocation(original) => original.before_length(),
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
            NativeUpdateBody::Fitted(original)
            | NativeUpdateBody::OwnedGrowth(original)
            | NativeUpdateBody::EofAllocation(original) => {
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
            (
                NativeUpdateBody::Fitted(original),
                NativeUpdateProfile::Ace16RetainedBooleanMaterialization,
            ) => original.encode_retained_boolean_materialization(budget, cancelled),
            (
                NativeUpdateBody::Fitted(original),
                NativeUpdateProfile::Ace16RetainedWideValueRows,
            ) => original.encode_retained_wide_values(budget, cancelled),
            (
                NativeUpdateBody::Fitted(original),
                NativeUpdateProfile::Ace16RetainedFixedScalarRows,
            ) => original.encode_retained_fixed_scalars(budget, cancelled),
            (NativeUpdateBody::Fitted(original), NativeUpdateProfile::Ace16RetainedBooleanRows) => {
                original.encode_retained_boolean(budget, cancelled)
            }
            (
                NativeUpdateBody::Fitted(original),
                NativeUpdateProfile::Ace16RetainedCurrencyRows,
            ) => original.encode_retained_currency(budget, cancelled),
            (NativeUpdateBody::Fitted(original), NativeUpdateProfile::Ace16RetainedOpaqueRows) => {
                original.encode_retained_opaque(budget, cancelled)
            }
            (NativeUpdateBody::Fitted(original), NativeUpdateProfile::Ace16RetainedRows) => {
                original.encode_retained(budget, cancelled)
            }
            (NativeUpdateBody::Fitted(original), NativeUpdateProfile::Ace16CurrentRows) => {
                original.encode_profile(super::fitted::WireProfile::CurrentRows, budget, cancelled)
            }
            (
                NativeUpdateBody::Fixed(original),
                NativeUpdateProfile::Ace16HistoricalForwardedNumberMaterialization,
            ) => original.encode_historical_forwarded_materialization(budget, cancelled),
            (
                NativeUpdateBody::Fixed(original),
                NativeUpdateProfile::Ace16HistoricalMixedNumberMaterialization,
            ) => original.encode_historical_mixed_materialization(budget, cancelled),
            (
                NativeUpdateBody::Fixed(original),
                NativeUpdateProfile::Ace16HistoricalNumberMaterialization,
            ) => original.encode_historical_materialization(budget, cancelled),
            (
                NativeUpdateBody::Fixed(original),
                NativeUpdateProfile::Ace16HistoricalPresentFixed,
            ) => original.encode_historical_present(budget, cancelled),
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
            (NativeUpdateBody::Fixed(original), NativeUpdateProfile::Ace16OrdinaryIeeeFixed) => {
                original.encode_ieee(budget, cancelled)
            }
            (NativeUpdateBody::Fixed(original), NativeUpdateProfile::Ace16OrdinaryScalarFixed) => {
                original.encode_scalar(budget, cancelled)
            }
            (NativeUpdateBody::Fixed(original), NativeUpdateProfile::Ace16OrdinaryGuidFixed) => {
                original.encode_guid(budget, cancelled)
            }
            (
                NativeUpdateBody::Fitted(original),
                NativeUpdateProfile::Ace16HistoricalShortTextUpdates,
            ) => original.encode_profile(
                super::fitted::WireProfile::HistoricalShortText,
                budget,
                cancelled,
            ),
            (NativeUpdateBody::Fitted(original), NativeUpdateProfile::Ace16OrdinaryFitted) => {
                original.encode(budget, cancelled)
            }
            (
                NativeUpdateBody::OwnedGrowth(original),
                NativeUpdateProfile::Ace16OrdinaryOwnedGrowth,
            ) => original.encode_profile(super::fitted::WireProfile::OwnedOnly, budget, cancelled),
            (
                NativeUpdateBody::EofAllocation(original),
                NativeUpdateProfile::Ace16OrdinaryEofAllocation,
            ) => {
                original.encode_profile(super::fitted::WireProfile::AllocateEof, budget, cancelled)
            }
            _ => Err(NativeIntentError::UnsupportedProfile),
        }
    }

    /// Decode exactly one version/profile; no catch/retry or cross-route fallback.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        if bytes.get(..super::fitted::RETAINED_BOOLEAN_MATERIALIZATION_MAGIC.len())
            == Some(super::fitted::RETAINED_BOOLEAN_MATERIALIZATION_MAGIC.as_slice())
        {
            return NativeFittedEditIntent::decode_retained_boolean_materialization(
                bytes, budget, cancelled,
            )
            .map(Self::from_retained_boolean_materialization);
        }
        if bytes.get(..super::fitted::RETAINED_WIDE_VALUE_ROWS_MAGIC.len())
            == Some(super::fitted::RETAINED_WIDE_VALUE_ROWS_MAGIC.as_slice())
        {
            return NativeFittedEditIntent::decode_retained_wide_values(bytes, budget, cancelled)
                .map(Self::from_retained_wide_value_rows);
        }
        if bytes.get(..super::fitted::RETAINED_FIXED_SCALAR_ROWS_MAGIC.len())
            == Some(super::fitted::RETAINED_FIXED_SCALAR_ROWS_MAGIC.as_slice())
        {
            return NativeFittedEditIntent::decode_retained_fixed_scalars(bytes, budget, cancelled)
                .map(Self::from_retained_fixed_scalar_rows);
        }
        if bytes.get(..super::fitted::RETAINED_BOOLEAN_ROWS_MAGIC.len())
            == Some(super::fitted::RETAINED_BOOLEAN_ROWS_MAGIC.as_slice())
        {
            return NativeFittedEditIntent::decode_retained_boolean(bytes, budget, cancelled)
                .map(Self::from_retained_boolean_rows);
        }
        if bytes.get(..super::fitted::RETAINED_CURRENCY_ROWS_MAGIC.len())
            == Some(super::fitted::RETAINED_CURRENCY_ROWS_MAGIC.as_slice())
        {
            return NativeFittedEditIntent::decode_retained_currency(bytes, budget, cancelled)
                .map(Self::from_retained_currency_rows);
        }
        if bytes.get(..super::fitted::RETAINED_OPAQUE_ROWS_MAGIC.len())
            == Some(super::fitted::RETAINED_OPAQUE_ROWS_MAGIC.as_slice())
        {
            return Ok(Self::from_retained_opaque_rows(
                NativeFittedEditIntent::decode_retained_opaque(bytes, budget, cancelled)?,
            ));
        }
        if bytes.get(..super::fitted::RETAINED_ROWS_MAGIC.len())
            == Some(super::fitted::RETAINED_ROWS_MAGIC.as_slice())
        {
            return Ok(Self::from_retained_rows(
                NativeFittedEditIntent::decode_retained(bytes, budget, cancelled)?,
            ));
        }
        if bytes.get(..super::fitted::CURRENT_ROWS_MAGIC.len())
            == Some(super::fitted::CURRENT_ROWS_MAGIC.as_slice())
        {
            return Ok(Self::from_current_rows(
                NativeFittedEditIntent::decode_profile(
                    bytes,
                    super::fitted::WireProfile::CurrentRows,
                    budget,
                    cancelled,
                )?,
            ));
        }
        if bytes.get(..super::fitted::HISTORICAL_TEXT_MAGIC.len())
            == Some(super::fitted::HISTORICAL_TEXT_MAGIC.as_slice())
        {
            return Ok(Self::from_historical_short_text_updates(
                NativeFittedEditIntent::decode_profile(
                    bytes,
                    super::fitted::WireProfile::HistoricalShortText,
                    budget,
                    cancelled,
                )?,
            ));
        }
        if bytes.get(..NativeFixedEditIntent::FORWARDED_MATERIALIZATION_MAGIC.len())
            == Some(NativeFixedEditIntent::FORWARDED_MATERIALIZATION_MAGIC.as_slice())
        {
            return Ok(Self::from_historical_forwarded_number_materialization(
                NativeFixedEditIntent::decode_historical_forwarded_materialization(
                    bytes, budget, cancelled,
                )?,
            ));
        }
        if bytes.get(..NativeFixedEditIntent::MIXED_MATERIALIZATION_MAGIC.len())
            == Some(NativeFixedEditIntent::MIXED_MATERIALIZATION_MAGIC.as_slice())
        {
            return Ok(Self::from_historical_mixed_number_materialization(
                NativeFixedEditIntent::decode_historical_mixed_materialization(
                    bytes, budget, cancelled,
                )?,
            ));
        }
        if bytes.get(..NativeFixedEditIntent::MATERIALIZATION_MAGIC.len())
            == Some(NativeFixedEditIntent::MATERIALIZATION_MAGIC.as_slice())
        {
            return Ok(Self::from_historical_number_materialization(
                NativeFixedEditIntent::decode_historical_materialization(bytes, budget, cancelled)?,
            ));
        }
        if bytes.get(..NativeFixedEditIntent::HISTORICAL_MAGIC.len())
            == Some(NativeFixedEditIntent::HISTORICAL_MAGIC.as_slice())
        {
            return Ok(Self::from_historical_present_fixed(
                NativeFixedEditIntent::decode_historical_present(bytes, budget, cancelled)?,
            ));
        }
        if bytes.get(..super::fitted::EOF_ALLOCATION_MAGIC.len())
            == Some(super::fitted::EOF_ALLOCATION_MAGIC.as_slice())
        {
            return Ok(Self::from_eof_allocation(
                NativeFittedEditIntent::decode_profile(
                    bytes,
                    super::fitted::WireProfile::AllocateEof,
                    budget,
                    cancelled,
                )?,
            ));
        }
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
        if bytes.get(..NativeFixedEditIntent::GUID_MAGIC.len())
            == Some(NativeFixedEditIntent::GUID_MAGIC.as_slice())
        {
            return Ok(Self::from_ordinary_guid_fixed(
                NativeFixedEditIntent::decode_guid(bytes, budget, cancelled)?,
            ));
        }
        if bytes.get(..NativeFixedEditIntent::SCALAR_MAGIC.len())
            == Some(NativeFixedEditIntent::SCALAR_MAGIC.as_slice())
        {
            return Ok(Self::from_ordinary_scalar_fixed(
                NativeFixedEditIntent::decode_scalar(bytes, budget, cancelled)?,
            ));
        }
        if bytes.get(..NativeFixedEditIntent::IEEE_MAGIC.len())
            == Some(NativeFixedEditIntent::IEEE_MAGIC.as_slice())
        {
            return Ok(Self::from_ordinary_ieee_fixed(
                NativeFixedEditIntent::decode_ieee(bytes, budget, cancelled)?,
            ));
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
