use super::super::PayloadPresence;
use tablecraft_core::ids::ObjectId;

/// One inert native binary payload. Undeclared columns, NULL and empty stored
/// bytes remain distinct; no VBA/OLE/application content is interpreted.
pub struct OpaquePayload {
    pub(in crate::preservation) presence: PayloadPresence,
    pub(in crate::preservation) bytes: Option<Vec<u8>>,
}
impl OpaquePayload {
    /// Physical catalog-column presence, without a dependency/safety conclusion.
    pub const fn presence(&self) -> PayloadPresence {
        self.presence
    }
    /// Exact assembled payload bytes when present, including a valid empty
    /// inline payload. None means NULL or uninspected, distinguished by presence.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

/// Completed opaque catalog payloads for an exact frozen-image object identity.
/// No Debug/Serde exposes bytes. This is not executable content or a write handle.
pub struct ObjectPayloadData {
    pub(super) object: ObjectId,
    pub(super) body: OpaquePayload,
    pub(super) module: OpaquePayload,
    pub(super) extra: OpaquePayload,
}
impl ObjectPayloadData {
    /// Exact catalog-scoped identity used to select this immutable record.
    pub const fn object_id(&self) -> ObjectId {
        self.object
    }
    /// Native Lv bytes, without interpreting the object's body format.
    pub fn body(&self) -> &OpaquePayload {
        &self.body
    }
    /// Native LvModule bytes, without VBA decoding or execution.
    pub fn module(&self) -> &OpaquePayload {
        &self.module
    }
    /// Native LvExtra bytes; dependencies and semantics remain uninspected.
    pub fn extra(&self) -> &OpaquePayload {
        &self.extra
    }
}
