use tablecraft_core::ids::ObjectId;

/// Exact source-bound native field identity, separate from displayed position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeFieldRenameIdentity {
    pub(super) object: ObjectId,
    pub(super) native_table: i32,
    pub(super) definition: u32,
    pub(super) number: usize,
    pub(super) column_id: u16,
    pub(super) name_units: usize,
}

impl NativeFieldRenameIdentity {
    /// Caller-scoped native table identity from this immutable original image.
    pub const fn object_id(self) -> ObjectId {
        self.object
    }

    /// Actual signed native table catalog identity.
    pub const fn native_table_id(self) -> i32 {
        self.native_table
    }

    /// Actual original native table-definition root page.
    pub const fn definition_page(self) -> u32 {
        self.definition
    }

    /// Native logical column number, never a caption or visible vector index.
    pub const fn column_number(self) -> usize {
        self.number
    }

    /// Original stored creation-time column ID, retained independently of order.
    pub const fn column_id(self) -> u16 {
        self.column_id
    }

    /// Qualified original name width required by this same-width profile.
    pub const fn name_utf16_units(self) -> usize {
        self.name_units
    }
}
