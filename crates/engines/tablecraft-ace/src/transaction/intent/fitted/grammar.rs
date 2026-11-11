//! Data grammar bounds, independent of consuming native write eligibility.
use super::{carrier, fixed_scalar_carrier, model::Family, wide_value_carrier};
use crate::{
    NativeImage,
    transaction::intent::{NativeIntentError, binding, model as names},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Grammar {
    Legacy,
    Retained,
    RetainedFixedScalars,
    RetainedWideValues,
}

impl Grammar {
    pub(super) const fn record_bytes(self) -> usize {
        match self {
            Self::Legacy => super::codec::MAX_RECORD_BYTES,
            Self::Retained | Self::RetainedFixedScalars | Self::RetainedWideValues => {
                u32::MAX as usize
            }
        }
    }

    pub(super) const fn assignments(self) -> usize {
        match self {
            Self::Legacy => NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS,
            Self::Retained | Self::RetainedFixedScalars | Self::RetainedWideValues => {
                u32::MAX as usize
            }
        }
    }

    pub(super) const fn groups(self) -> usize {
        match self {
            Self::Legacy => NativeImage::MAX_FIXED_BATCH_GROUPS,
            Self::Retained | Self::RetainedFixedScalars | Self::RetainedWideValues => {
                u32::MAX as usize
            }
        }
    }

    pub(super) fn name(
        self,
        name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        if self == Self::Legacy {
            return names::validate_name(name, budget, cancelled);
        }
        names::validate_retained_name(name, budget, cancelled)
    }

    pub(super) fn length(self, length: u64) -> Result<(), NativeIntentError> {
        match self {
            Self::Legacy => binding::length(length),
            Self::Retained | Self::RetainedFixedScalars | Self::RetainedWideValues => {
                binding::retained_length(length)
            }
        }
    }

    pub(super) fn digest(
        self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<[u8; 32], NativeIntentError> {
        match self {
            Self::Legacy => binding::digest(before, budget, cancelled),
            Self::Retained | Self::RetainedFixedScalars | Self::RetainedWideValues => {
                binding::digest_retained(before, budget, cancelled)
            }
        }
    }

    pub(super) fn view(
        self,
        value: &Value,
        family: Family,
    ) -> Result<carrier::Encoded<'_>, NativeIntentError> {
        match self {
            Self::Legacy => carrier::view(value, family),
            Self::Retained => carrier::length_prefixed_view(value, family),
            Self::RetainedFixedScalars => fixed_scalar_carrier::view(value, family),
            Self::RetainedWideValues => wide_value_carrier::view(value, family),
        }
    }

    pub(super) fn parse<'a>(
        self,
        tag: u8,
        bytes: &'a [u8],
        family: Family,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<carrier::Borrowed<'a>, NativeIntentError> {
        match self {
            Self::Legacy => carrier::parse(tag, bytes, family, budget, cancelled),
            Self::Retained => carrier::length_prefixed_parse(tag, bytes, family, budget, cancelled),
            Self::RetainedFixedScalars => {
                fixed_scalar_carrier::parse(tag, bytes, family, budget, cancelled)
            }
            Self::RetainedWideValues => {
                wide_value_carrier::parse(tag, bytes, family, budget, cancelled)
            }
        }
    }
}
