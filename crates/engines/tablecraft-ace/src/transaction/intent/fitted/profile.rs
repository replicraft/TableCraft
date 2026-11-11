//! Closed data-record choices; no tag is a native allocation permission.
use super::{
    codec::{MAGIC, PROFILE},
    grammar::Grammar,
};

pub(in crate::transaction::intent) const OWNED_GROWTH_MAGIC: &[u8; 8] = b"TCNI0006";
const OWNED_GROWTH_PROFILE: u16 = 6;
pub(in crate::transaction::intent) const EOF_ALLOCATION_MAGIC: &[u8; 8] = b"TCNI0007";
const EOF_ALLOCATION_PROFILE: u16 = 7;
pub(in crate::transaction::intent) const HISTORICAL_TEXT_MAGIC: &[u8; 8] = b"TCNI0015";
const HISTORICAL_TEXT_PROFILE: u16 = 15;
pub(in crate::transaction::intent) const CURRENT_ROWS_MAGIC: &[u8; 8] = b"TCNI0016";
const CURRENT_ROWS_PROFILE: u16 = 16;

pub(in crate::transaction::intent) const RETAINED_ROWS_MAGIC: &[u8; 8] = b"TCNI0017";
const RETAINED_ROWS_PROFILE: u16 = 17;
pub(in crate::transaction::intent) const RETAINED_OPAQUE_ROWS_MAGIC: &[u8; 8] = b"TCNI0018";
const RETAINED_OPAQUE_ROWS_PROFILE: u16 = 18;
pub(in crate::transaction::intent) const RETAINED_CURRENCY_ROWS_MAGIC: &[u8; 8] = b"TCNI0019";
const RETAINED_CURRENCY_ROWS_PROFILE: u16 = 19;
pub(in crate::transaction::intent) const RETAINED_BOOLEAN_ROWS_MAGIC: &[u8; 8] = b"TCNI0020";
const RETAINED_BOOLEAN_ROWS_PROFILE: u16 = 20;
pub(in crate::transaction::intent) const RETAINED_FIXED_SCALAR_ROWS_MAGIC: &[u8; 8] = b"TCNI0021";
const RETAINED_FIXED_SCALAR_ROWS_PROFILE: u16 = 21;

pub(in crate::transaction::intent) const RETAINED_WIDE_VALUE_ROWS_MAGIC: &[u8; 8] = b"TCNI0022";
const RETAINED_WIDE_VALUE_ROWS_PROFILE: u16 = 22;
pub(in crate::transaction::intent) const RETAINED_BOOLEAN_MATERIALIZATION_MAGIC: &[u8; 8] =
    b"TCNI0023";
const RETAINED_BOOLEAN_MATERIALIZATION_PROFILE: u16 = 23;

pub(in crate::transaction::intent) const RETAINED_EMPTY_ROWS_MAGIC: &[u8; 8] = b"TCNI0024";
const RETAINED_EMPTY_ROWS_PROFILE: u16 = 24;

#[derive(Clone, Copy)]
pub(in crate::transaction::intent) enum WireProfile {
    Fitted,
    OwnedOnly,
    AllocateEof,
    HistoricalShortText,
    CurrentRows,
    RetainedRows,
    RetainedOpaqueRows,
    RetainedCurrencyRows,
    RetainedBooleanRows,
    RetainedFixedScalarRows,
    RetainedWideValueRows,
    RetainedBooleanMaterialization,
    RetainedEmptyRows,
}

impl WireProfile {
    pub(super) const fn grammar(self) -> Grammar {
        match self {
            Self::RetainedEmptyRows => Grammar::RetainedEmpty,
            Self::RetainedFixedScalarRows => Grammar::RetainedFixedScalars,
            Self::RetainedWideValueRows => Grammar::RetainedWideValues,
            Self::RetainedRows
            | Self::RetainedOpaqueRows
            | Self::RetainedCurrencyRows
            | Self::RetainedBooleanRows
            | Self::RetainedBooleanMaterialization => Grammar::Retained,
            _ => Grammar::Legacy,
        }
    }

    pub(super) const fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::RetainedEmptyRows => RETAINED_EMPTY_ROWS_MAGIC,
            Self::Fitted => MAGIC,
            Self::OwnedOnly => OWNED_GROWTH_MAGIC,
            Self::AllocateEof => EOF_ALLOCATION_MAGIC,
            Self::HistoricalShortText => HISTORICAL_TEXT_MAGIC,
            Self::CurrentRows => CURRENT_ROWS_MAGIC,
            Self::RetainedRows => RETAINED_ROWS_MAGIC,
            Self::RetainedOpaqueRows => RETAINED_OPAQUE_ROWS_MAGIC,
            Self::RetainedCurrencyRows => RETAINED_CURRENCY_ROWS_MAGIC,
            Self::RetainedBooleanRows => RETAINED_BOOLEAN_ROWS_MAGIC,
            Self::RetainedFixedScalarRows => RETAINED_FIXED_SCALAR_ROWS_MAGIC,
            Self::RetainedWideValueRows => RETAINED_WIDE_VALUE_ROWS_MAGIC,
            Self::RetainedBooleanMaterialization => RETAINED_BOOLEAN_MATERIALIZATION_MAGIC,
        }
    }

    pub(super) const fn number(self) -> u16 {
        match self {
            Self::RetainedEmptyRows => RETAINED_EMPTY_ROWS_PROFILE,
            Self::Fitted => PROFILE,
            Self::OwnedOnly => OWNED_GROWTH_PROFILE,
            Self::AllocateEof => EOF_ALLOCATION_PROFILE,
            Self::HistoricalShortText => HISTORICAL_TEXT_PROFILE,
            Self::CurrentRows => CURRENT_ROWS_PROFILE,
            Self::RetainedRows => RETAINED_ROWS_PROFILE,
            Self::RetainedOpaqueRows => RETAINED_OPAQUE_ROWS_PROFILE,
            Self::RetainedCurrencyRows => RETAINED_CURRENCY_ROWS_PROFILE,
            Self::RetainedBooleanRows => RETAINED_BOOLEAN_ROWS_PROFILE,
            Self::RetainedFixedScalarRows => RETAINED_FIXED_SCALAR_ROWS_PROFILE,
            Self::RetainedWideValueRows => RETAINED_WIDE_VALUE_ROWS_PROFILE,
            Self::RetainedBooleanMaterialization => RETAINED_BOOLEAN_MATERIALIZATION_PROFILE,
        }
    }
}
