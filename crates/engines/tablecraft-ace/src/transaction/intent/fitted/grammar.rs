//! Data grammar bounds, independent of consuming native write eligibility.
use super::{carrier, model::Family};
use crate::{
    InspectError, NativeImage,
    header::check,
    transaction::intent::{NativeIntentError, binding, model as names},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Grammar {
    Legacy,
    Retained,
}

impl Grammar {
    pub(super) const fn record_bytes(self) -> usize {
        match self {
            Self::Legacy => super::codec::MAX_RECORD_BYTES,
            Self::Retained => u32::MAX as usize,
        }
    }

    pub(super) const fn assignments(self) -> usize {
        match self {
            Self::Legacy => NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS,
            Self::Retained => u32::MAX as usize,
        }
    }

    pub(super) const fn groups(self) -> usize {
        match self {
            Self::Legacy => NativeImage::MAX_FIXED_BATCH_GROUPS,
            Self::Retained => u32::MAX as usize,
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
        check(budget, cancelled)?;
        if name.is_empty() || u32::try_from(name.len()).is_err() {
            return Err(NativeIntentError::InvalidRecord);
        }
        for chunk in name.as_bytes().chunks(carrier::COPY_BYTES) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, chunk.len() as u64)
                .map_err(InspectError::from)?;
        }
        for (position, character) in name.chars().enumerate() {
            if position % carrier::COPY_BYTES == 0 {
                check(budget, cancelled)?;
            }
            if character.is_control() {
                return Err(NativeIntentError::InvalidRecord);
            }
        }
        check(budget, cancelled)?;
        Ok(())
    }

    pub(super) fn length(self, length: u64) -> Result<(), NativeIntentError> {
        match self {
            Self::Legacy => binding::length(length),
            Self::Retained => binding::retained_length(length),
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
            Self::Retained => binding::digest_retained(before, budget, cancelled),
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
        }
    }
}
