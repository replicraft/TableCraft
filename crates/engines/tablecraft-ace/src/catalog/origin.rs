use tablecraft_core::ids::ObjectId;

/// Storage declared by a native table catalog entry, not a readability guarantee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTableStorage {
    /// Local table definition/records in this database.
    Local,
    /// Non-ODBC linked table; its provider/source format is not inspected.
    Linked,
    /// Linked ODBC table; no connection is attempted during inspection.
    LinkedOdbc,
}

/// Native metadata for an inspected descriptor. This reference is meaningful
/// only in its original stable source; it is not a mutation handle or global ID.
pub struct NativeObjectOrigin {
    pub(super) object: ObjectId,
    pub(super) native_id: i32,
    pub(super) parent_id: i32,
    pub(super) type_code: i16,
    pub(super) flags: u32,
    pub(super) page: u32,
    pub(super) slot: u8,
}
impl NativeObjectOrigin {
    /// Scoped semantic identity used by the model catalog.
    pub const fn object_id(&self) -> ObjectId {
        self.object
    }
    /// Unmodified signed MSysObjects Id, separate from semantic identity.
    pub const fn native_id(&self) -> i32 {
        self.native_id
    }
    /// Unmodified native containment reference.
    pub const fn parent_id(&self) -> i32 {
        self.parent_id
    }
    /// Exact native signed type code, including unrecognized types.
    pub const fn type_code(&self) -> i16 {
        self.type_code
    }
    /// Recognized table storage from the captured type code, without reading
    /// connection properties. Non-table/unknown types return None.
    pub const fn table_storage(&self) -> Option<NativeTableStorage> {
        match self.type_code {
            super::record::LOCAL_TABLE => Some(NativeTableStorage::Local),
            super::record::LINKED_TABLE => Some(NativeTableStorage::Linked),
            super::record::ODBC_TABLE => Some(NativeTableStorage::LinkedOdbc),
            _ => None,
        }
    }
    /// Uninterpreted native flags; no credentials or property streams are read.
    pub const fn flags(&self) -> u32 {
        self.flags
    }
    /// Logical catalog root page containing this record or its relocation pointer.
    pub const fn page(&self) -> u32 {
        self.page
    }
    /// Logical row slot on that page.
    pub const fn slot(&self) -> u8 {
        self.slot
    }
}
