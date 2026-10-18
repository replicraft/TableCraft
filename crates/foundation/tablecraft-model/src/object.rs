use tablecraft_core::ids::ObjectId;
use tablecraft_schema::SchemaName;

/// Shared category, without native numeric type codes or detailed domain models.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    /// Table, including linked-table metadata; no linked source is opened.
    Table,
    /// Saved query metadata, not parsed/executed SQL.
    Query,
    /// Form metadata, without layout/control decoding or rendering.
    Form,
    /// Report metadata, without sections or print behavior.
    Report,
    /// Macro metadata, without language decoding/execution.
    Macro,
    /// Module metadata, without source extraction/execution.
    Module,
    /// Native/application catalog grouping container.
    Container,
    /// Relationship object identity, without relational key/schema decoding.
    Relationship,
    /// Native kind not mapped by the adapter; retain its source metadata there.
    Unknown,
}
/// Validated common object metadata. No Debug/Serde exposes names or contents.
/// Parent references may be unresolved; they are containment, not a complete
/// dependency graph or proof that dynamic references have been discovered.
#[derive(Clone)]
pub struct ObjectDescriptor {
    id: ObjectId,
    name: SchemaName,
    kind: ObjectKind,
    parent: Option<ObjectId>,
}
impl ObjectDescriptor {
    /// Compose typed metadata; catalog construction validates identity scope.
    pub fn new(id: ObjectId, name: SchemaName, kind: ObjectKind, parent: Option<ObjectId>) -> Self {
        Self {
            id,
            name,
            kind,
            parent,
        }
    }
    /// Semantic identity issued in the catalog namespace.
    pub const fn id(&self) -> ObjectId {
        self.id
    }
    /// Validated original spelling.
    pub fn name(&self) -> &SchemaName {
        &self.name
    }
    /// Shared object category.
    pub const fn kind(&self) -> ObjectKind {
        self.kind
    }
    /// Parent object identity, potentially absent from the current inventory.
    pub const fn parent(&self) -> Option<ObjectId> {
        self.parent
    }
}
