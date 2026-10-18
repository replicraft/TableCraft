use crate::{InspectError, format::data_type};
use tablecraft_core::ids::ObjectId;

/// Native property-block scope. Column and index maps may share a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyGroupKind {
    /// Unnamed table-level property map.
    Table,
    /// Named column-level map.
    Column,
    /// Named index-level map.
    Index,
    /// Retained block tag with unqualified semantics.
    Unknown(u16),
}

/// One inert native property. Diagnostic formatting intentionally omits values.
pub struct NativeProperty {
    pub(super) name: String,
    pub(super) kind: u8,
    pub(super) flags: u8,
    pub(super) raw: Vec<u8>,
    pub(super) text: Option<String>,
}

impl NativeProperty {
    /// Original property key.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Original native type tag, including unqualified types.
    pub const fn native_type(&self) -> u8 {
        self.kind
    }
    /// Uninterpreted property flags; retained independently of the type tag.
    pub const fn flags(&self) -> u8 {
        self.flags
    }
    /// Original value bytes, not a semantic value or reconstructed encoding.
    pub fn raw_value(&self) -> &[u8] {
        &self.raw
    }
    /// Decode a qualified one-byte property Boolean. This is not a row null mask.
    pub fn boolean(&self) -> Result<bool, InspectError> {
        if self.kind != data_type::BOOLEAN {
            return Err(InspectError::Unsupported);
        }
        match self.raw.as_slice() {
            [value] => Ok(*value != 0),
            _ => Err(InspectError::Malformed),
        }
    }
    /// Decoded text, never evaluated as a default, validation rule or callback.
    pub fn text(&self) -> Result<&str, InspectError> {
        self.text.as_deref().ok_or(InspectError::Unsupported)
    }
}

/// Scoped properties; absent keys remain absent rather than fabricated defaults.
pub struct PropertyGroup {
    pub(super) kind: PropertyGroupKind,
    pub(super) name: String,
    pub(super) properties: Vec<NativeProperty>,
}

impl PropertyGroup {
    /// Property scope, distinct from a column/index name.
    pub const fn kind(&self) -> PropertyGroupKind {
        self.kind
    }
    /// Scope name; the table-level group is unnamed.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Properties in native block order, including unknown keys/types.
    pub fn properties(&self) -> &[NativeProperty] {
        &self.properties
    }
    /// ASCII-insensitive key lookup; not a full Access locale comparison.
    pub fn get(&self, name: &str) -> Option<&NativeProperty> {
        self.properties
            .iter()
            .find(|property| property.name.eq_ignore_ascii_case(name))
    }
}

/// Completed table/column/index property inspection, with the complete original
/// LvProp bytes retained. This is metadata, not native save authorization.
pub struct TableProperties {
    pub(super) name: String,
    pub(super) object: ObjectId,
    pub(super) raw: Vec<u8>,
    pub(super) groups: Vec<PropertyGroup>,
}

impl TableProperties {
    /// Original table name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Caller-scoped object identity.
    pub const fn object_id(&self) -> ObjectId {
        self.object
    }
    /// Full original property stream, including unknown flags/types and padding.
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }
    /// Native property groups, without executing their contents.
    pub fn groups(&self) -> &[PropertyGroup] {
        &self.groups
    }
    /// Find a group by scope and ASCII-insensitive name.
    pub fn group(&self, kind: PropertyGroupKind, name: &str) -> Option<&PropertyGroup> {
        self.groups
            .iter()
            .find(|group| group.kind == kind && group.name.eq_ignore_ascii_case(name))
    }
}
