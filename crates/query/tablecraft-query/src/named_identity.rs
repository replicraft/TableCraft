//! A definition's owning namespace is part of its identity, not its SQL name.
use crate::{external_sources::SourceNamespace, profile::SOURCE_SCHEMA_POLICY};

pub(crate) struct NamedIdentity {
    pub namespace: SourceNamespace,
    pub name: String,
}

impl NamedIdentity {
    pub fn matches(&self, namespace: SourceNamespace, name: &str) -> bool {
        self.namespace == namespace && SOURCE_SCHEMA_POLICY.names.equals(&self.name, name)
    }
}

/// Metadata discovered before binding requested an execution-dependent schema.
/// Keep its external revision evidence even though no complete plan exists yet.
pub(crate) struct SchemaRequest {
    pub identity: NamedIdentity,
    pub external_sources: Vec<crate::external_sources::ExternalBinding>,
}
