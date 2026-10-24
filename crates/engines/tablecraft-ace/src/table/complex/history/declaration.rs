use crate::NativeSortOrder;
use tablecraft_schema::{CollectionMember, SchemaName};

/// An explicit stored-history member on its actual hidden parent field.
/// The visible Memo association is resolved from metadata, never a generated name.
pub struct VersionHistoryMemberDeclaration {
    pub(super) member: CollectionMember,
    pub(super) memo: SchemaName,
    pub(super) order: Option<NativeSortOrder>,
}

impl VersionHistoryMemberDeclaration {
    /// Aligned member declaration using stored hidden/value-field names.
    pub fn member(&self) -> &CollectionMember {
        &self.member
    }

    /// Associated visible current-value Memo; not a history member alias.
    pub fn memo_name(&self) -> &SchemaName {
        &self.memo
    }

    /// Exact flat Text descriptor, or None for non-Text members.
    pub const fn text_sort_order(&self) -> Option<NativeSortOrder> {
        self.order
    }

    /// Consume metadata without cloning validated names or declarations.
    pub fn into_parts(self) -> (CollectionMember, SchemaName, Option<NativeSortOrder>) {
        (self.member, self.memo, self.order)
    }
}
