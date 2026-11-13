//! Move-only names and independently proved QueryDef images.
use super::{NativeQueryCreationCandidate, NativeQueryStorageChange, QueryCreationDefinition};
use crate::NativeImage;

pub(crate) struct NativeQueryCreationParts {
    pub(crate) definition: QueryCreationDefinition,
    pub(crate) effect: NativeQueryCreationEffect,
}

/// Only the qualified QueryDef constructor can produce this immutable effect.
pub(crate) struct NativeQueryCreationEffect {
    before: NativeImage,
    after: NativeImage,
    native_id: i32,
    storage_change: NativeQueryStorageChange,
}

impl NativeQueryCreationEffect {
    pub(crate) const fn storage_change(&self) -> NativeQueryStorageChange {
        self.storage_change
    }

    pub(crate) fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    pub(crate) fn after_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }

    pub(crate) fn after_shared_bytes(&self) -> bytes::Bytes {
        // NO_CLONE: Fresh requalification compares this immutable AFTER after
        // dropping the old effect; the paid shared Bytes handle copies no payload.
        self.after.bytes.clone()
    }

    pub(crate) const fn native_query_id(&self) -> i32 {
        self.native_id
    }
}

impl NativeQueryCreationCandidate {
    pub(crate) fn into_creation_parts(self) -> NativeQueryCreationParts {
        NativeQueryCreationParts {
            definition: self.definition,
            effect: NativeQueryCreationEffect {
                before: self.before,
                after: self.after,
                native_id: self.native_id,
                storage_change: self.storage_change,
            },
        }
    }
}
