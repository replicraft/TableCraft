use super::super::PayloadPresence;
use tablecraft_core::ids::ObjectId;

/// Structural coverage of one inert catalog LvExtra payload. None of these
/// states certifies data-macro absence, executable semantics or write safety.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableExtraCoverage {
    /// The catalog field is missing or NULL; consult physical presence.
    NotPresent,
    /// Present payload with zero bytes.
    Empty,
    /// Complete framing in the observed wide-MR2 profile, possibly names only.
    WideMr2,
    /// Original bytes retained, with an unsupported signature or opaque content.
    Opaque,
}

/// One property in an observed wide-MR2 group. Names and numeric type/flags are
/// uninterpreted native facts; text is never parsed as XML or executed.
pub struct TableExtraProperty {
    pub(super) name: String,
    pub(super) native_type: u8,
    pub(super) flags: u8,
    pub(super) raw: Vec<u8>,
    pub(super) text: Option<String>,
}

impl TableExtraProperty {
    /// Exact name selected by the original name-list ordinal.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Original native property type, without conversion into an event type.
    pub const fn native_type(&self) -> u8 {
        self.native_type
    }

    /// Original uninterpreted property flags.
    pub const fn flags(&self) -> u8 {
        self.flags
    }

    /// Exact property value bytes.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Strict UTF-16LE text for observed flags/type, without XML interpretation.
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

/// One original numeric group and its exact body. Unknown groups remain opaque;
/// no group number or property name is promoted into a data-macro event enum.
pub struct TableExtraGroup {
    pub(super) kind: u16,
    pub(super) raw: Vec<u8>,
    pub(super) properties: Option<Vec<TableExtraProperty>>,
}

impl TableExtraGroup {
    /// Original numeric group kind.
    pub const fn kind(&self) -> u16 {
        self.kind
    }

    /// Original complete block body, excluding its length and kind header.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Parsed properties when group framing is supported; None stays opaque.
    pub fn properties(&self) -> Option<&[TableExtraProperty]> {
        self.properties.as_deref()
    }
}

/// An inert inspection bound to one local table in a complete immutable image.
/// Missing catalog columns, NULL and present-empty bytes remain distinct. No
/// Debug/Serde implementation publishes stored application text implicitly.
pub struct TableExtra {
    pub(super) object: ObjectId,
    pub(super) native_id: i32,
    pub(super) parent_id: i32,
    pub(super) catalog_page: u32,
    pub(super) catalog_slot: u8,
    pub(super) presence: PayloadPresence,
    pub(super) raw: Option<Vec<u8>>,
    pub(super) decoded: super::decode::Decoded,
}

impl TableExtra {
    /// Exact catalog-scoped identity used to select the table.
    pub const fn object_id(&self) -> ObjectId {
        self.object
    }

    /// Original signed MSysObjects Id, separate from scoped semantic identity.
    pub const fn native_id(&self) -> i32 {
        self.native_id
    }

    /// Original signed MSysObjects ParentId.
    pub const fn parent_id(&self) -> i32 {
        self.parent_id
    }

    /// Logical catalog row root page, including when its record is forwarded.
    pub const fn catalog_page(&self) -> u32 {
        self.catalog_page
    }

    /// Logical catalog row slot.
    pub const fn catalog_slot(&self) -> u8 {
        self.catalog_slot
    }

    /// Physical catalog field presence, without an absence or safety conclusion.
    pub const fn presence(&self) -> PayloadPresence {
        self.presence
    }

    /// Exact assembled LvExtra bytes. None means missing or NULL, not empty.
    pub fn raw(&self) -> Option<&[u8]> {
        self.raw.as_deref()
    }

    /// Complete/opaque framing coverage, never macro execution qualification.
    pub const fn coverage(&self) -> TableExtraCoverage {
        self.decoded.coverage
    }

    /// Complete original property-name list, when its framing was supported.
    pub fn names(&self) -> &[String] {
        &self.decoded.names
    }

    /// Numeric value groups, including groups retained as opaque bytes.
    pub fn groups(&self) -> &[TableExtraGroup] {
        &self.decoded.groups
    }
}
