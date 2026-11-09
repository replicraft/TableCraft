//! Exact external database tokens resolve only to supplied native handles.
mod error;
mod forwarding;
mod mapping;
pub use error::ExternalSourceError;
pub use mapping::NativeExternalMapping;

use super::{LinkSourceError, LinkedNativeSource, NativeSourceHandle};
use std::cell::RefCell;
use tablecraft_ace::NativeTableStorage;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::{
    QueryError, Source,
    source::{ExternalSourceRequest, ResolvedExternalSource, SourceKey},
};

struct Target<'a> {
    database: &'a str,
    key: SourceKey,
    source: LocalSource<'a>,
}

struct LocalSource<'a> {
    handle: NativeSourceHandle<'a>,
}

enum Root<'a> {
    Native(NativeSourceHandle<'a>),
    Linked(&'a LinkedNativeSource<'a>),
}

impl<'a> Root<'a> {
    fn handle(&self) -> NativeSourceHandle<'a> {
        match self {
            Self::Native(handle) => *handle,
            Self::Linked(source) => source.root_handle(),
        }
    }

    fn source(&self) -> &'a dyn Source {
        match self {
            Self::Native(handle) => handle.source(),
            Self::Linked(source) => *source,
        }
    }

    fn validate(&self, target: NativeSourceHandle<'a>) -> Result<(), LinkSourceError> {
        match self {
            Self::Native(root) if root.same_source(target) => {
                root.check()?;
                target.check()?;
                Ok(())
            }
            Self::Native(root) => super::links::validate_external_handles(*root, target),
            Self::Linked(root) => root.validate_external_target(target),
        }
    }
}

/// Explicit native Access IN bindings over stable, already-open sources.
/// Root tables, saved queries and mapped links keep the root namespace. External
/// targets expose local base tables and qualified saved read definitions. Linked
/// target tables remain refused. No embedded token opens a path, loads
/// a provider, changes credentials or establishes a cross-file snapshot.
pub struct ExternalNativeSource<'a> {
    root: Root<'a>,
    targets: Vec<Target<'a>>,
}

impl<'a> ExternalNativeSource<'a> {
    /// Bind exact tokens to native handles without reading user records.
    /// Every handle must share the root's borrowed native pool, and distinct
    /// handles must have distinct caller-issued catalog scopes.
    pub fn new(
        root: NativeSourceHandle<'a>,
        mappings: &[NativeExternalMapping<'a>],
    ) -> Result<Self, ExternalSourceError> {
        Self::configure(Root::Native(root), mappings)
    }

    /// Preserve an existing explicit linked-table facade as the root namespace.
    /// External handles also validate against the linked facade's target scopes.
    pub fn with_links(
        root: &'a LinkedNativeSource<'a>,
        mappings: &[NativeExternalMapping<'a>],
    ) -> Result<Self, ExternalSourceError> {
        Self::configure(Root::Linked(root), mappings)
    }

    fn configure(
        root: Root<'a>,
        mappings: &[NativeExternalMapping<'a>],
    ) -> Result<Self, ExternalSourceError> {
        let _span = tracing::debug_span!("native.external.source.configure").entered();
        let handle = root.handle();
        handle.check()?;
        let pool = handle
            .shared_budget()
            .ok_or(LinkSourceError::SharedBudgetRequired)?;
        debit(
            handle,
            pool,
            Resource::Bytes,
            mappings
                .len()
                .checked_mul(size_of::<Target<'_>>())
                .ok_or(QueryError::StructureLimit)?,
        )?;
        let mut targets: Vec<Target<'a>> = Vec::new();
        targets
            .try_reserve_exact(mappings.len())
            .map_err(|_| QueryError::Allocation)?;

        for mapping in mappings {
            debit(handle, pool, Resource::Steps, 1)?;
            root.validate(mapping.target)?;
            for previous in &targets {
                debit(
                    handle,
                    pool,
                    Resource::Steps,
                    previous
                        .database
                        .len()
                        .saturating_add(mapping.database.len())
                        .saturating_add(1),
                )?;
                if previous.database == mapping.database {
                    return Err(ExternalSourceError::DuplicateToken);
                }
                if previous.key == mapping.key {
                    return Err(ExternalSourceError::DuplicateKey);
                }
                if previous.source.handle.scope() == mapping.target.scope()
                    && !previous.source.handle.same_source(mapping.target)
                {
                    return Err(LinkSourceError::ScopeCollision.into());
                }
            }
            targets.push(Target {
                database: mapping.database,
                key: mapping.key,
                source: LocalSource {
                    handle: mapping.target,
                },
            });
        }

        handle.check()?;
        Ok(Self { root, targets })
    }

    fn resolve(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        let handle = self.root.handle();
        handle.check()?;
        if request.connection().is_some() || request.connection_bracketed() {
            return Err(QueryError::Unsupported);
        }
        let pool = handle.shared_budget().ok_or(QueryError::Source)?;
        for target in &self.targets {
            debit(
                handle,
                pool,
                Resource::Steps,
                target
                    .database
                    .len()
                    .saturating_add(request.database().len())
                    .saturating_add(1),
            )?;
            if target.database == request.database() {
                target.source.handle.check()?;
                return Ok(Some(ResolvedExternalSource::new(
                    target.key,
                    &target.source,
                )));
            }
        }
        Ok(None)
    }
}

impl<'a> LocalSource<'a> {
    fn route(&self, table: &str) -> Result<(&'a dyn Source, &'a str), QueryError> {
        self.handle.check()?;
        let identity = self.handle.table_identity(table)?;
        if identity.storage() != NativeTableStorage::Local {
            return Err(QueryError::Unsupported);
        }
        Ok((self.handle.source(), identity.name()))
    }
}

fn debit(
    handle: NativeSourceHandle<'_>,
    pool: &RefCell<ResourceBudget>,
    resource: Resource,
    count: usize,
) -> Result<(), QueryError> {
    handle.check()?;
    pool.try_borrow_mut()
        .map_err(|_| QueryError::Source)?
        .charge(resource, count as u64)?;
    Ok(())
}
