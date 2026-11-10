//! Distinct native source proofs share one original-request journal owner.
use super::codec::Profile;
use crate::{
    NativeCurrentTailFieldDropCandidate, NativeFieldDropCandidate, NativeFieldDropIdentity,
};

pub(super) enum Candidate {
    Empty(NativeFieldDropCandidate),
    PopulatedTail(NativeCurrentTailFieldDropCandidate),
}

impl Candidate {
    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Empty(candidate) => candidate.before_bytes(),
            Self::PopulatedTail(candidate) => candidate.before_bytes(),
        }
    }

    pub(super) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Empty(candidate) => candidate.after_bytes(),
            Self::PopulatedTail(candidate) => candidate.after_bytes(),
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::Empty(candidate) => candidate.table(),
            Self::PopulatedTail(candidate) => candidate.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::Empty(candidate) => candidate.column(),
            Self::PopulatedTail(candidate) => candidate.column(),
        }
    }

    pub(super) fn identity(&self) -> NativeFieldDropIdentity {
        match self {
            Self::Empty(candidate) => candidate.identity(),
            Self::PopulatedTail(candidate) => candidate.identity(),
        }
    }

    pub(super) fn profile(&self) -> Profile {
        match self {
            Self::Empty(_) => Profile::Empty,
            Self::PopulatedTail(_) => Profile::PopulatedTail,
        }
    }
}
