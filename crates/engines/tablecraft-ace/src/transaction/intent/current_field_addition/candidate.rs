//! One current-source proof owner, with explicit semantic declaration branches.
use crate::{
    FieldAdditionTarget, NativeCurrentFieldAdditionCandidate,
    NativeCurrentShortTextFieldAdditionCandidate,
};

pub(super) enum Candidate {
    Number(NativeCurrentFieldAdditionCandidate),
    ShortText(NativeCurrentShortTextFieldAdditionCandidate),
}

impl Candidate {
    pub(super) fn is_retained(&self) -> bool {
        match self {
            Self::Number(candidate) => candidate.is_retained(),
            Self::ShortText(candidate) => candidate.is_retained(),
        }
    }

    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Number(candidate) => candidate.before_bytes(),
            Self::ShortText(candidate) => candidate.before_bytes(),
        }
    }

    pub(super) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Number(candidate) => candidate.after_bytes(),
            Self::ShortText(candidate) => candidate.after_bytes(),
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::Number(candidate) => candidate.table(),
            Self::ShortText(candidate) => candidate.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::Number(candidate) => candidate.column(),
            Self::ShortText(candidate) => candidate.column(),
        }
    }

    pub(super) fn target(&self) -> FieldAdditionTarget {
        match self {
            Self::Number(_) => FieldAdditionTarget::NumberLongZero,
            Self::ShortText(candidate) => FieldAdditionTarget::ShortText {
                max_utf16_units: candidate.max_utf16_units(),
                unicode_compression: candidate.unicode_compression(),
            },
        }
    }
}
