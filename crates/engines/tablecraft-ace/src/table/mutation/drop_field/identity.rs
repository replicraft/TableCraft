use super::super::saved_field::Identity;
use tablecraft_core::ids::ObjectId;

/// Exact original field identity, separate from a visible position or caption.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeFieldDropIdentity {
    object: ObjectId,
    native_table: i32,
    definition: u32,
    number: usize,
    column_id: u16,
}

impl NativeFieldDropIdentity {
    pub(super) const fn from_observed(value: Identity) -> Self {
        Self {
            object: value.object,
            native_table: value.native_table,
            definition: value.definition,
            number: value.number,
            column_id: value.column_id,
        }
    }

    /// Caller-scoped original table object identity.
    pub const fn object_id(self) -> ObjectId {
        self.object
    }

    /// Actual signed native catalog identity.
    pub const fn native_table_id(self) -> i32 {
        self.native_table
    }

    /// Actual original table-definition root page.
    pub const fn definition_page(self) -> u32 {
        self.definition
    }

    /// Native logical column number, never a visible vector position.
    pub const fn column_number(self) -> usize {
        self.number
    }

    /// Stored creation-time column ID, retained independently of native number.
    pub const fn column_id(self) -> u16 {
        self.column_id
    }
}
