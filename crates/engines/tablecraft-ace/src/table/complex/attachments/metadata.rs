use super::{record, schema::Columns};
use crate::{AttachmentDecoder, ComplexColumnLinks, DecodedAttachment, InspectError};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{LegacyDateTime, Value},
};

/// Complete checked native attachment collections with encoded file data.
pub struct AttachmentColumn {
    pub(super) links: ComplexColumnLinks,
    pub(super) columns: Columns,
}
impl AttachmentColumn {
    pub(in crate::table::complex) fn record<'a>(
        &self,
        row: &'a [tablecraft_core::values::Value],
    ) -> Result<StoredAttachment<'a>, InspectError> {
        record::read(row, &self.columns)
    }
    /// Raw checked schema/identity links; parent keys are snapshot-local.
    pub fn links(&self) -> &ComplexColumnLinks {
        &self.links
    }

    /// Borrow an exact stored member, including FileData's encoded Binary envelope.
    /// No attachment decoder runs. NULL, empty bytes and unknown wrappers remain
    /// distinct; unknown names and absent parent/child ordinals return None.
    pub fn stored_value(&self, parent: usize, child: usize, member: &str) -> Option<&Value> {
        let position = super::schema::FIELDS
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(member))?;
        self.links
            .child_row(parent, child)?
            .get(self.columns.positions[position])
    }

    /// Borrow a qualified scalar metadata member at a checked child ordinal.
    /// FileData and unknown names return None; stored NULL remains Some(Null).
    pub fn metadata_value(&self, parent: usize, child: usize, member: &str) -> Option<&Value> {
        let position = super::schema::FIELDS
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(member))?;
        if position == super::schema::DATA {
            return None;
        }
        self.links
            .child_row(parent, child)?
            .get(self.columns.positions[position])
    }

    /// Exact flat text order for a qualified metadata member. Unknown names,
    /// FileData and non-Text members return None; unknown descriptors remain
    /// explicit. This reads no records and never supplies a comparison policy.
    pub fn metadata_text_sort_order(&self, member: &str) -> Option<crate::NativeSortOrder> {
        let position = super::schema::FIELDS
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(member))?;
        if position == super::schema::DATA {
            return None;
        }
        self.links.child_snapshot().schema().columns()[self.columns.positions[position]]
            .text_sort_order()
    }

    /// Borrow stored metadata/payloads in flat snapshot order. All records were
    /// checked before publication; accessors still return typed errors rather
    /// than relying on unchecked casts. An absent parent ordinal returns None.
    pub fn attachments(
        &self,
        parent: usize,
    ) -> Option<impl ExactSizeIterator<Item = Result<StoredAttachment<'_>, InspectError>>> {
        Some(
            self.links
                .child_rows(parent)?
                .map(|row| record::read(row, &self.columns)),
        )
    }
}

/// Borrowed stored attachment. Metadata strings are not validated paths or MIME
/// types; encoded bytes are not automatically decoded, opened or executed.
pub struct StoredAttachment<'a> {
    pub(super) identity: i32,
    pub(super) url: Option<&'a str>,
    pub(super) name: Option<&'a str>,
    pub(super) file_type: Option<&'a str>,
    pub(super) encoded: Option<&'a [u8]>,
    pub(super) timestamp: Option<LegacyDateTime>,
    pub(super) flags: Option<i32>,
}
impl<'a> StoredAttachment<'a> {
    /// Signed stored child identity, scoped to this flat snapshot.
    pub const fn identity(&self) -> i32 {
        self.identity
    }
    /// Stored URL metadata; never resolved by this reader.
    pub const fn file_url(&self) -> Option<&'a str> {
        self.url
    }
    /// Stored name metadata; never used as an extraction destination.
    pub const fn file_name(&self) -> Option<&'a str> {
        self.name
    }
    /// Stored field type label, independent of the encoded wrapper's label.
    pub const fn file_type(&self) -> Option<&'a str> {
        self.file_type
    }
    /// Native wrapped bytes, preserving NULL separately from an empty byte value.
    pub const fn encoded_file_data(&self) -> Option<&'a [u8]> {
        self.encoded
    }
    /// Exact stored legacy DateTime serial, without timezone conversion.
    pub const fn file_timestamp(&self) -> Option<LegacyDateTime> {
        self.timestamp
    }
    /// Raw stored flags; no actions or extraction policy are inferred.
    pub const fn file_flags(&self) -> Option<i32> {
        self.flags
    }
    /// Explicit bounded wrapper decoding. A NULL stored payload returns None;
    /// a present invalid/unsupported wrapper returns an error, never empty bytes.
    pub fn decode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<DecodedAttachment>, InspectError> {
        self.decode_with(&mut AttachmentDecoder::new(budget, cancelled))
    }

    /// Decode with a reusable workspace bound to the caller's original controls.
    /// NULL payloads still return None without allocating a codec workspace.
    pub fn decode_with(
        &self,
        decoder: &mut AttachmentDecoder<'_>,
    ) -> Result<Option<DecodedAttachment>, InspectError> {
        decoder.decode_optional(self.encoded)
    }
}
