use super::super::NativeSourceHandle;

/// One explicit host binding from a stored root alias to an already-open table.
/// Strings are exact stored identifiers, not paths to open or credentials to use.
/// Both native IDs are signed, source-local catalog identities. Handles and their
/// caller-issued scopes remain immutable for the facade's lifetime.
pub struct NativeLinkMapping<'a> {
    pub(super) alias: &'a str,
    pub(super) native_id: i32,
    pub(super) database: &'a str,
    pub(super) target: NativeSourceHandle<'a>,
    pub(super) target_table: &'a str,
    pub(super) target_native_id: i32,
}

impl<'a> NativeLinkMapping<'a> {
    /// Pin the local declaration and the exact selected target table.
    /// Construction stores identifiers only. `LinkedNativeSource::new` validates
    /// them against native metadata and requires the same borrowed native pool.
    pub const fn new(
        alias: &'a str,
        native_id: i32,
        database: &'a str,
        target: NativeSourceHandle<'a>,
        target_table: &'a str,
        target_native_id: i32,
    ) -> Self {
        Self {
            alias,
            native_id,
            database,
            target,
            target_table,
            target_native_id,
        }
    }
}
