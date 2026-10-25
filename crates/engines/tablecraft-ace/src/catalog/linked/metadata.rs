use crate::{InspectError, NativeTableStorage};
use tablecraft_core::ids::ObjectId;

/// Borrowed native catalog field, retaining missing/NULL/unsupported distinctions.
/// Text may contain sensitive connection metadata and is never filesystem authority.
pub enum NativeLinkText<'a> {
    /// The catalog does not declare this field.
    Missing,
    /// The field is declared but its stored value is NULL.
    Null,
    /// Exact decoded stored Text; no connection grammar or path is interpreted.
    Text(&'a str),
    /// The field declaration is outside the qualified native Text profile.
    Unsupported,
}

pub(super) enum OwnedText {
    Missing,
    Null,
    Text(String),
    Unsupported,
}

impl OwnedText {
    pub(super) fn borrowed(&self) -> NativeLinkText<'_> {
        match self {
            Self::Missing => NativeLinkText::Missing,
            Self::Null => NativeLinkText::Null,
            Self::Text(text) => NativeLinkText::Text(text),
            Self::Unsupported => NativeLinkText::Unsupported,
        }
    }
}

/// Completed inert linked-table catalog metadata. No target is opened or resolved.
/// No Debug/Serde implementation exposes paths or connection strings implicitly.
pub struct NativeLinkedTableDefinition {
    pub(super) object: ObjectId,
    pub(super) native_id: i32,
    pub(super) name: String,
    pub(super) storage: NativeTableStorage,
    pub(super) database: OwnedText,
    pub(super) foreign_name: OwnedText,
    pub(super) connect: OwnedText,
}

impl NativeLinkedTableDefinition {
    /// Caller-scoped local catalog object identity.
    pub const fn object_id(&self) -> ObjectId {
        self.object
    }

    /// Exact signed local MSysObjects Id, not a target database identity.
    pub const fn native_id(&self) -> i32 {
        self.native_id
    }

    /// Local linked-table alias, without resolving its remote name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Native linked/non-ODBC or linked-ODBC storage declaration.
    pub const fn storage(&self) -> NativeTableStorage {
        self.storage
    }

    /// Raw native Database field; never opened, normalized or resolved.
    pub fn database(&self) -> NativeLinkText<'_> {
        self.database.borrowed()
    }

    /// Raw native ForeignName field; never parsed as SQL or a saved query.
    pub fn foreign_name(&self) -> NativeLinkText<'_> {
        self.foreign_name.borrowed()
    }

    /// Raw native Connect field; potentially sensitive, never used as credentials.
    pub fn connect(&self) -> NativeLinkText<'_> {
        self.connect.borrowed()
    }

    /// Borrow qualified Access-file target identifiers without resolving them.
    /// Requires non-ODBC storage, nonempty Database/ForeignName and an exactly
    /// NULL or empty Connect field. Missing/unsupported/provider-specific fields
    /// remain visible through the raw getters but cannot establish this profile.
    /// Hosts must separately authorize and validate any target/path/table lookup.
    pub fn access_target(&self) -> Result<NativeAccessLinkTarget<'_>, InspectError> {
        if self.storage != NativeTableStorage::Linked {
            return Err(InspectError::Unsupported);
        }
        let (NativeLinkText::Text(database), NativeLinkText::Text(table)) =
            (self.database(), self.foreign_name())
        else {
            return Err(InspectError::Unsupported);
        };
        if database.is_empty() || table.is_empty() {
            return Err(InspectError::Unsupported);
        }
        if !matches!(
            self.connect(),
            NativeLinkText::Null | NativeLinkText::Text("")
        ) {
            return Err(InspectError::Unsupported);
        }
        Ok(NativeAccessLinkTarget { database, table })
    }
}

/// Opaque stored Access-file target identifiers, with no open/authorization claim.
pub struct NativeAccessLinkTarget<'a> {
    database: &'a str,
    table: &'a str,
}

impl<'a> NativeAccessLinkTarget<'a> {
    /// Exact stored Database string, not a validated host path.
    pub const fn database(&self) -> &'a str {
        self.database
    }

    /// Exact stored ForeignName string, not executable SQL.
    pub const fn table(&self) -> &'a str {
        self.table
    }
}
