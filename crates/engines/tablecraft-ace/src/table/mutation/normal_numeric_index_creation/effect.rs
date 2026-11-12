//! Numeric-only request names move separately from the proved native effect.
use super::NativeNumericIndexCandidate;
use crate::{NativeImage, table::mutation::index_population::Built};
use tablecraft_schema::{ScalarType, SchemaName};

pub(crate) struct NativeNumericIndexCreateParts {
    pub(crate) effect: NativeNumericIndexCreateEffect,
    pub(crate) table: String,
    pub(crate) column: String,
    pub(crate) index: SchemaName,
}

/// Only the qualified numeric constructor can produce this immutable image pair.
pub(crate) struct NativeNumericIndexCreateEffect {
    before: NativeImage,
    after: NativeImage,
    scalar: ScalarType,
    native_table: i32,
    definition: u32,
    native: usize,
    column_id: u16,
    descending: bool,
    logical: u32,
    physical: u32,
    rows: usize,
}

impl NativeNumericIndexCreateEffect {
    pub(crate) fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    pub(crate) fn after_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }

    pub(crate) fn after_shared_bytes(&self) -> bytes::Bytes {
        // NO_CLONE: The complete read-only AFTER witness can outlive this effect.
        // Sharing its already paid immutable Bytes handle copies no payload.
        self.after.bytes.clone()
    }

    pub(crate) const fn scalar_type(&self) -> ScalarType {
        self.scalar
    }

    pub(crate) const fn native_table_id(&self) -> i32 {
        self.native_table
    }

    pub(crate) const fn definition_page(&self) -> u32 {
        self.definition
    }

    pub(crate) const fn native_number(&self) -> usize {
        self.native
    }

    pub(crate) const fn column_id(&self) -> u16 {
        self.column_id
    }

    pub(crate) const fn descending(&self) -> bool {
        self.descending
    }

    pub(crate) const fn logical_index_number(&self) -> u32 {
        self.logical
    }

    pub(crate) const fn physical_index_number(&self) -> u32 {
        self.physical
    }

    pub(crate) const fn indexed_rows(&self) -> usize {
        self.rows
    }
}

impl NativeNumericIndexCandidate {
    pub(crate) fn into_numeric_parts(self) -> NativeNumericIndexCreateParts {
        let Built {
            before,
            after,
            scalar,
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
        } = self.built;

        NativeNumericIndexCreateParts {
            effect: NativeNumericIndexCreateEffect {
                before,
                after,
                scalar,
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
