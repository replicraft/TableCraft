use crate::NativeTableRenameIdentity;
use tablecraft_core::ids::ObjectId;

/// Exact original identity of a logically deleted ordinary local table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTableDropIdentity {
    object: ObjectId,
    native: i32,
    parent: i32,
    definition: u32,
    catalog_page: u32,
    catalog_slot: u8,
}

impl NativeTableDropIdentity {
    pub(super) fn from_source(source: NativeTableRenameIdentity) -> Self {
        Self {
            object: source.object_id(),
            native: source.native_id(),
            parent: source.parent_id(),
            definition: source.definition_page(),
            catalog_page: source.catalog_page(),
            catalog_slot: source.catalog_slot(),
        }
    }

    /// Caller-scoped object identity from the captured source catalog.
    pub const fn object_id(self) -> ObjectId {
        self.object
    }

    /// Original signed native MSysObjects Id.
    pub const fn native_id(self) -> i32 {
        self.native
    }

    /// Original native containment identity.
    pub const fn parent_id(self) -> i32 {
        self.parent
    }

    /// Original TDEF page, retained as historical storage after logical deletion.
    pub const fn definition_page(self) -> u32 {
        self.definition
    }

    /// Original catalog DATA page containing the selected root slot.
    pub const fn catalog_page(self) -> u32 {
        self.catalog_page
    }

    /// Original stable catalog slot, which becomes a deleted root tombstone.
    pub const fn catalog_slot(self) -> u8 {
        self.catalog_slot
    }
}
