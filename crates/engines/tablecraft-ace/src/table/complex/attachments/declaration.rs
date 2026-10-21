use crate::NativeSortOrder;
use tablecraft_schema::CollectionMember;

/// Qualified stored attachment member metadata. FileData stays absent/opaque.
/// Text order belongs to the actual flat member; template/database settings do
/// not substitute for unknown descriptors and no comparison policy activates.
pub struct AttachmentMemberDeclaration {
    pub(super) member: CollectionMember,
    pub(super) order: Option<NativeSortOrder>,
}
impl AttachmentMemberDeclaration {
    /// Shared aligned member declaration, named within its parent collection.
    pub fn member(&self) -> &CollectionMember {
        &self.member
    }
    /// Exact flat Text member order, or None for a non-Text carrier.
    pub const fn text_sort_order(&self) -> Option<NativeSortOrder> {
        self.order
    }
    /// Consume the native wrapper without cloning the shared declaration.
    pub fn into_member(self) -> CollectionMember {
        self.member
    }
}
