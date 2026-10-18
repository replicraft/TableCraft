use tablecraft_core::ids::ObjectId;

/// Caller-issued catalog namespace. Callers own uniqueness and persistence.
/// It is not a native database ID, file hash, UUID or revision/generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogScope(u64);
impl CatalogScope {
    /// Select an explicit namespace; never reuse it for unrelated catalogs.
    pub const fn new(namespace: u64) -> Self {
        Self(namespace)
    }
    /// Map a source-local token into this scope without losing its bits.
    /// The adapter owns the native-to-local mapping and token lifecycle.
    pub const fn object_id(self, local: u64) -> ObjectId {
        ObjectId::from_u128(((self.0 as u128) << u64::BITS) | (local as u128))
    }
    /// Whether an identity belongs to this namespace.
    pub const fn contains(self, id: ObjectId) -> bool {
        (id.as_u128() >> u64::BITS) == self.0 as u128
    }
}
