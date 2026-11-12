//! Consuming request/effect split used by the fresh original owner.
use super::NativeLongIndexCandidate;
use crate::NativeImage;
use crate::table::mutation::index_population_source::Construction;
use tablecraft_schema::SchemaName;

pub(crate) struct NativeIndexCreateParts {
    pub(crate) effect: NativeIndexCreateEffect,
    pub(crate) table: String,
    pub(crate) column: String,
    pub(crate) index: SchemaName,
}

/// Only the successful native constructor can create this immutable effect.
pub(crate) struct NativeIndexCreateEffect {
    before: NativeImage,
    after: NativeImage,
    construction: Construction,
    policy: crate::table::mutation::index_creation::IndexPolicy,
    native_table: i32,
    definition: u32,
    native: usize,
    column_id: u16,
    descending: bool,
    logical: u32,
    physical: u32,
    rows: usize,
}
impl NativeIndexCreateEffect {
    pub(crate) fn is_unique_construction(&self) -> bool {
        matches!(
            self.policy,
            crate::table::mutation::index_creation::IndexPolicy::UniqueLong
        )
    }

    pub(crate) fn is_fresh_construction(&self) -> bool {
        self.construction == Construction::Fresh
    }

    pub(crate) fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    pub(crate) fn after_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }

    pub(crate) fn after_shared_bytes(&self) -> bytes::Bytes {
        // NO_CLONE: The readonly complete-AFTER witness outlives this effect;
        // only its paid immutable control handle is copied, never the payload.
        self.after.bytes.clone()
    }

    pub(crate) fn native_table_id(&self) -> i32 {
        self.native_table
    }

    pub(crate) fn definition_page(&self) -> u32 {
        self.definition
    }

    pub(crate) fn native_number(&self) -> usize {
        self.native
    }

    pub(crate) fn column_id(&self) -> u16 {
        self.column_id
    }

    pub(crate) fn descending(&self) -> bool {
        self.descending
    }

    pub(crate) fn logical_index_number(&self) -> u32 {
        self.logical
    }

    pub(crate) fn physical_index_number(&self) -> u32 {
        self.physical
    }

    pub(crate) fn indexed_rows(&self) -> usize {
        self.rows
    }
}
impl NativeLongIndexCandidate {
    pub(crate) fn into_parts(self) -> NativeIndexCreateParts {
        let Self {
            before,
            after,
            construction,
            policy,
            table,
            column,
            index,
            native_table,
            definition,
            native,
            column_id,
            descending,
            logical,
            physical,
            rows,
            ..
        } = self;
        NativeIndexCreateParts {
            effect: NativeIndexCreateEffect {
                before,
                after,
                construction,
                policy,
                native_table,
                definition,
                native,
                column_id,
                descending,
                logical,
                physical,
                rows,
            },
            table,
            column,
            index,
        }
    }
}
