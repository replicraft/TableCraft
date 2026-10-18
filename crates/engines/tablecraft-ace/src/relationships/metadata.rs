use tablecraft_schema::Relationship;

/// Native relationship origin plus validated backend-neutral declaration shape.
/// No rows are checked, providers activated or cascade actions executed.
pub struct NativeRelationship {
    pub(super) declaration: Relationship,
    pub(super) flags: u32,
}
impl NativeRelationship {
    /// Validated ordered parent/dependent mapping and optional rule semantics.
    pub fn declaration(&self) -> &Relationship {
        &self.declaration
    }
    /// Original grbit bits, including unknown/unqualified declarations.
    pub const fn flags(&self) -> u32 {
        self.flags
    }
}
