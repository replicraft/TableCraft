use crate::{NativeImage, NativeObjectOrigin, ObjectPayloadData, OpaquePayload, TableExtra};
use tablecraft_core::ids::ObjectId;
use tablecraft_model::ObjectDescriptor;

/// One observed child candidate and its retained, uninterpreted payloads.
pub struct MacroCoverageChild<'image> {
    pub(super) object: ObjectId,
    pub(super) descriptor: &'image ObjectDescriptor,
    pub(super) origin: &'image NativeObjectOrigin,
    pub(super) payloads: ObjectPayloadData,
}

impl MacroCoverageChild<'_> {
    /// Exact catalog-scoped identity of this retained direct child.
    pub const fn object_id(&self) -> ObjectId {
        self.object
    }

    /// Shared descriptor from the same captured catalog, including unknown kinds.
    pub const fn descriptor(&self) -> &ObjectDescriptor {
        self.descriptor
    }

    /// Exact native child identity, flags and catalog record location.
    pub const fn origin(&self) -> &NativeObjectOrigin {
        self.origin
    }

    /// All three original payload roles, retaining uninspected/NULL/empty states.
    pub fn payloads(&self) -> &ObjectPayloadData {
        &self.payloads
    }
}

/// Source-bound inert coverage of a table and its observed child candidates.
/// This cannot establish required-event discovery, absence or save permission.
pub struct TableMacroCoverage<'image> {
    pub(super) image: &'image NativeImage,
    pub(super) origin: &'image NativeObjectOrigin,
    pub(super) body: OpaquePayload,
    pub(super) module: OpaquePayload,
    pub(super) extra: TableExtra,
    pub(super) children: Vec<MacroCoverageChild<'image>>,
}

impl<'image> TableMacroCoverage<'image> {
    /// Exact captured source used by every retained observation.
    pub const fn image(&self) -> &'image NativeImage {
        self.image
    }

    /// Selected local table origin in this report's captured source.
    pub const fn table_origin(&self) -> &NativeObjectOrigin {
        self.origin
    }

    /// Exact opaque table Lv payload and its physical presence.
    pub fn body(&self) -> &OpaquePayload {
        &self.body
    }

    /// Exact opaque table LvModule payload, without interpreting VBA.
    pub fn module(&self) -> &OpaquePayload {
        &self.module
    }

    /// Retained table LvExtra bytes and bounded framing observations.
    pub fn table_extra(&self) -> &TableExtra {
        &self.extra
    }

    /// Semantic direct children in source order, checked against native ParentId.
    /// Child streams remain opaque even when bytes resemble macro XML.
    pub fn children(&self) -> &[MacroCoverageChild<'_>] {
        &self.children
    }
}
