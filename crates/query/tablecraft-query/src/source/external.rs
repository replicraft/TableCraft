//! Explicit host-authorized source bindings; database tokens never authorize opens.
use super::Source;

/// Caller-issued identity of one immutable external binding/revision.
/// This is not a physical file identity, path, credential or permission token.
/// Change it when the authorized target or binding revision changes, and never
/// reuse it for distinct bindings during a prepared query's lifetime.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceKey(u128);

impl SourceKey {
    /// Wrap a caller-issued immutable binding identity without discovering sources.
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Read the opaque value for explicit host mapping and revision checks.
    pub const fn get(self) -> u128 {
        self.0
    }
}

/// Exact external database/connection syntax requested by an Access IN clause.
/// No normalization, connection parsing or implicit filesystem authority is added.
#[derive(Clone, Copy)]
pub struct ExternalSourceRequest<'a> {
    database: &'a str,
    connection: Option<&'a str>,
    connection_bracketed: bool,
}

impl<'a> ExternalSourceRequest<'a> {
    /// Borrow exact owned query strings without opening or interpreting them.
    pub const fn new(
        database: &'a str,
        connection: Option<&'a str>,
        connection_bracketed: bool,
    ) -> Self {
        Self {
            database,
            connection,
            connection_bracketed,
        }
    }

    /// Exact database token, never an instruction to open a path.
    pub const fn database(self) -> &'a str {
        self.database
    }

    /// Exact optional connection token; absent and empty remain distinct.
    pub const fn connection(self) -> Option<&'a str> {
        self.connection
    }

    /// Whether the connection token was bracket-delimited in the owned syntax.
    pub const fn connection_bracketed(self) -> bool {
        self.connection_bracketed
    }
}

/// Already-authorized borrowed provider plus its immutable binding identity.
/// The provider must remain stable throughout preparation and execution. Target
/// base tables are exposed through existing Source methods; external named query
/// expansion and recursive provider activation are not admitted by this contract.
pub struct ResolvedExternalSource<'a> {
    key: SourceKey,
    source: &'a dyn Source,
}

impl<'a> ResolvedExternalSource<'a> {
    /// Return an explicitly supplied source without opening or cloning it.
    pub const fn new(key: SourceKey, source: &'a dyn Source) -> Self {
        Self { key, source }
    }

    /// Immutable host binding/revision identity, not physical file evidence.
    pub const fn key(&self) -> SourceKey {
        self.key
    }

    /// Borrow the actual provider used for metadata and row callbacks.
    pub const fn source(&self) -> &'a dyn Source {
        self.source
    }
}
