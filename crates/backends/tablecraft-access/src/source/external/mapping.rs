use super::super::NativeSourceHandle;
use tablecraft_query::source::SourceKey;

/// An exact opaque database token bound to an already-open native source.
/// The key is host-issued and must change when the binding or source revision
/// changes. It is neither a filesystem identity nor authorization to open a path.
pub struct NativeExternalMapping<'a> {
    pub(super) database: &'a str,
    pub(super) key: SourceKey,
    pub(super) target: NativeSourceHandle<'a>,
}

impl<'a> NativeExternalMapping<'a> {
    /// Store a borrowed binding; the facade validates shared budgets and scopes.
    pub const fn new(database: &'a str, key: SourceKey, target: NativeSourceHandle<'a>) -> Self {
        Self {
            database,
            key,
            target,
        }
    }
}
