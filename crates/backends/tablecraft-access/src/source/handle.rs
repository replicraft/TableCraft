//! Borrowed native-only handles; callers cannot substitute arbitrary query providers.
use super::{NativeSource, budget};
use std::cell::RefCell;
use tablecraft_ace::{
    InspectError, NativeLinkedTableDefinition, NativeReadSource, NativeTableStorage,
    read_linked_table_definition,
};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, Source};

#[derive(Clone, Copy)]
pub(super) struct TableOrigin {
    pub(super) object: ObjectId,
    pub(super) native_id: i32,
    pub(super) storage: NativeTableStorage,
}

/// Borrowed inert catalog identity, not a filesystem identity or mutation handle.
/// Canonical names remain in the already-open source. No table records are read.
#[derive(Clone, Copy)]
pub struct NativeTableIdentity<'a> {
    name: &'a str,
    origin: TableOrigin,
}

impl<'a> NativeTableIdentity<'a> {
    /// Exact spelling captured from the inert catalog inventory.
    pub const fn name(self) -> &'a str {
        self.name
    }

    /// Caller-scoped semantic identity captured from this source's catalog.
    pub const fn object_id(self) -> ObjectId {
        self.origin.object
    }

    /// Unmodified signed native catalog ID; meaningful only in its source.
    pub const fn native_id(self) -> i32 {
        self.origin.native_id
    }

    /// Captured local/linked/ODBC declaration, without a readability guarantee.
    pub const fn storage(self) -> NativeTableStorage {
        self.origin.storage
    }
}

/// Borrow an already-open native source while erasing its concrete reader type.
/// Only `NativeSource::handle` constructs handles; arbitrary query providers
/// cannot be substituted. Copying a handle does not open, read or clone a source.
#[derive(Clone, Copy)]
pub struct NativeSourceHandle<'a> {
    inner: &'a dyn NativeHandle,
}

impl<'a> NativeSourceHandle<'a> {
    /// Caller-issued catalog scope, not a file identity or revision token.
    pub fn scope(self) -> CatalogScope {
        self.inner.scope()
    }

    /// Resolve a captured table identity without reading its schema or records.
    /// Uses the source's existing name comparison and returns canonical spelling.
    /// Non-table and unknown catalog kinds are not treated as local tables.
    pub fn table_identity(self, name: &str) -> Result<NativeTableIdentity<'a>, QueryError> {
        self.inner.table_identity(name)
    }

    /// Explicitly read one inert native linked-table declaration under this
    /// source's existing allowance and cancellation callback. Raw stored target
    /// identifiers remain data: this never opens a path or activates a provider.
    /// A failure returns no partial definition and refunds no successful charges.
    pub fn read_linked_table_definition(
        self,
        name: &str,
    ) -> Result<NativeLinkedTableDefinition, InspectError> {
        self.inner.linked_definition(name)
    }

    pub(super) fn source(self) -> &'a dyn Source {
        self.inner.source()
    }

    pub(super) fn shared_budget(self) -> Option<&'a RefCell<ResourceBudget>> {
        self.inner.shared_budget()
    }

    pub(super) fn check(self) -> Result<(), QueryError> {
        self.inner.check()
    }

    pub(super) fn same_source(self, other: NativeSourceHandle<'_>) -> bool {
        std::ptr::addr_eq(self.inner, other.inner)
    }
}

trait NativeHandle {
    fn source(&self) -> &dyn Source;
    fn scope(&self) -> CatalogScope;
    fn shared_budget(&self) -> Option<&RefCell<ResourceBudget>>;
    fn table_identity(&self, name: &str) -> Result<NativeTableIdentity<'_>, QueryError>;
    fn check(&self) -> Result<(), QueryError>;
    fn linked_definition(&self, name: &str) -> Result<NativeLinkedTableDefinition, InspectError>;
}

impl<R: NativeReadSource> NativeSource<'_, R> {
    /// Borrow a native-only handle without reading rows or opening linked targets.
    /// The handle retains this source's captured identities and native allowance.
    pub fn handle(&self) -> NativeSourceHandle<'_> {
        NativeSourceHandle { inner: self }
    }
}

impl<R: NativeReadSource> NativeHandle for NativeSource<'_, R> {
    fn source(&self) -> &dyn Source {
        self
    }

    fn scope(&self) -> CatalogScope {
        self.scope
    }

    fn shared_budget(&self) -> Option<&RefCell<ResourceBudget>> {
        match &self.budget {
            budget::Budget::Owned(_) => None,
            budget::Budget::Shared(pool) => Some(pool),
        }
    }

    fn table_identity(&self, name: &str) -> Result<NativeTableIdentity<'_>, QueryError> {
        let entry = self.entry(name)?;
        Ok(NativeTableIdentity {
            name: &entry.name,
            origin: entry.origin,
        })
    }

    fn check(&self) -> Result<(), QueryError> {
        if (self.cancelled)() {
            return Err(QueryError::Cancelled);
        }
        Ok(())
    }

    fn linked_definition(&self, name: &str) -> Result<NativeLinkedTableDefinition, InspectError> {
        if (self.cancelled)() {
            return Err(InspectError::Cancelled);
        }
        let mut source = self
            .source
            .try_borrow_mut()
            .map_err(|_| InspectError::Unsupported)?;
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| InspectError::Unsupported)?;
        read_linked_table_definition(&mut *source, self.scope, name, &mut budget, self.cancelled)
    }
}
