//! Plan-local collection lineage, separate from row/sidecar positions.
use crate::external_sources::SourceNamespace;
use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CollectionOrigin {
    pub namespace: SourceNamespace,
    pub occurrence: usize,
    pub member: usize,
}

impl CollectionOrigin {
    pub(crate) fn compare(self, other: Self) -> Ordering {
        fn namespace_key(namespace: SourceNamespace) -> (u8, u128) {
            match namespace {
                SourceNamespace::Root => (0, 0),
                SourceNamespace::External(key) => (1, key.get()),
            }
        }
        (namespace_key(self.namespace), self.occurrence, self.member).cmp(&(
            namespace_key(other.namespace),
            other.occurrence,
            other.member,
        ))
    }
}
