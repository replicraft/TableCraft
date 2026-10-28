use super::super::schema;
use crate::table::complex::links::membership::Membership;
use crate::{
    ComplexColumnSchema, ComplexParentLink, ComplexTableSchema, NativeSortOrder, TableSnapshot,
};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

/// Complete checked attachment membership and metadata projection. Encoded file
/// payloads are omitted, not represented by NULL, and remain unqualified.
pub struct AttachmentMetadataColumn {
    pub(super) schema: ComplexTableSchema,
    pub(super) position: usize,
    pub(super) membership: Membership,
    pub(super) children: TableSnapshot,
    pub(super) positions: [Option<usize>; schema::FIELD_COUNT],
}

impl AttachmentMetadataColumn {
    /// Original owner and complex declarations; no application content executes.
    pub fn schema(&self) -> &ComplexTableSchema {
        &self.schema
    }

    /// The selected attachment declaration, retaining native table references.
    pub fn column(&self) -> &ComplexColumnSchema {
        &self.schema.columns()[self.position]
    }

    /// Checked parent memberships in sequential live-root order.
    pub fn parents(&self) -> &[ComplexParentLink] {
        self.membership.parents()
    }

    /// Complete projected children with metadata and reference columns only.
    /// Native schema retains FileData's declaration; columns/rows omit its value.
    pub fn child_snapshot(&self) -> &TableSnapshot {
        &self.children
    }

    /// Certified scalar declaration of a named attachment member.
    /// FileData remains declared Binary even though its value is omitted and unvalidated.
    /// Reference columns and unknown member names return None.
    pub fn member_type(&self, member: &str) -> Option<ScalarType> {
        schema::member_type(member)
    }

    /// Borrow a metadata value at snapshot-local parent/child ordinals.
    /// Stored NULL is Some(Null); FileData and unknown/out-of-range names are None.
    pub fn metadata_value(&self, parent: usize, child: usize, member: &str) -> Option<&Value> {
        let role = schema::FIELDS
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(member))?;
        let position = self.positions[role]?;
        let ordinal = self.membership.child_ordinal(parent, child)?;
        self.children.rows().get(ordinal)?.get(position)
    }

    /// Exact native sort identity for a projected Text member; no resolver runs.
    pub fn metadata_text_sort_order(&self, member: &str) -> Option<NativeSortOrder> {
        let role = schema::FIELDS
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(member))?;
        self.positions[role]?;
        self.children
            .schema()
            .columns()
            .iter()
            .find(|field| field.name().eq_ignore_ascii_case(member))?
            .text_sort_order()
    }
}
