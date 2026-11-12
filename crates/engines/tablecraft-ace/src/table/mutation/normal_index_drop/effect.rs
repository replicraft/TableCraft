//! Actual DROP request ownership moves separately from its sealed native effect.
use super::NativeIndexDropCandidate;
use crate::NativeImage;

pub(crate) struct NativeIndexDropParts {
    pub(crate) effect: NativeIndexDropEffect,
    pub(crate) table: String,
    pub(crate) index: String,
}

/// No caller or decoded request can construct this proved image pair.
pub(crate) struct NativeIndexDropEffect {
    policy: super::source::Policy,
    before: NativeImage,
    after: NativeImage,
    native_table: i32,
    definition: u32,
    logical: u32,
    physical: u32,
    retired_pages: usize,
}

impl NativeIndexDropEffect {
    pub(crate) fn is_unique_drop(&self) -> bool {
        matches!(self.policy, super::source::Policy::UniqueLong)
    }

    pub(crate) fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    pub(crate) fn after_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }

    pub(crate) fn after_shared_bytes(&self) -> bytes::Bytes {
        // NO_CLONE: The independent readonly AFTER witness may outlive this
        // effect. This copies only its already paid immutable Bytes handle.
        self.after.bytes.clone()
    }

    pub(crate) fn native_table_id(&self) -> i32 {
        self.native_table
    }

    pub(crate) fn definition_page(&self) -> u32 {
        self.definition
    }

    pub(crate) fn logical_index_number(&self) -> u32 {
        self.logical
    }

    pub(crate) fn physical_index_number(&self) -> u32 {
        self.physical
    }

    pub(crate) fn retired_pages(&self) -> usize {
        self.retired_pages
    }
}

impl NativeIndexDropCandidate {
    pub(crate) fn into_parts(self) -> NativeIndexDropParts {
        let Self {
            policy,
            before,
            after,
            table,
            index,
            native_table,
            definition,
            logical,
            physical,
            retired_pages,
        } = self;
        NativeIndexDropParts {
            effect: NativeIndexDropEffect {
                policy,
                before,
                after,
                native_table,
                definition,
                logical,
                physical,
                retired_pages,
            },
            table,
            index,
        }
    }
}
