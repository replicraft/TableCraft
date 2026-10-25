//! Explicit one-hop routing over already-open stable native sources.
mod error;
mod forwarding;
mod mapping;
pub use error::LinkSourceError;
pub use mapping::NativeLinkMapping;

use super::NativeSourceHandle;
use std::cell::RefCell;
use tablecraft_ace::{InspectError, NativeTableStorage};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::{QueryError, Source};

struct Route<'a> {
    alias: &'a str,
    target: NativeSourceHandle<'a>,
    table: &'a str,
}

/// Immutable explicit routing from root Access links to already-open local tables.
/// Every source borrows one cumulative native allowance. Hosts own each stable
/// source, authentication and scope; no path, connection or provider is opened.
/// Named queries resolve only in the root namespace. Reprepare after changing
/// handles, mappings or source bytes; this is not a cross-file snapshot guarantee.
pub struct LinkedNativeSource<'a> {
    root: NativeSourceHandle<'a>,
    routes: Vec<Route<'a>>,
}

impl<'a> LinkedNativeSource<'a> {
    /// Validate supplied one-hop bindings without scanning user records.
    /// Supplied root links need exactly one mapping. Exact stored Database/ForeignName,
    /// native IDs and canonical names must agree. Targets must be local base tables,
    /// use distinct source scopes and share the root's borrowed native allowance.
    /// ODBC/provider links, chains, saved-query targets and surplus mappings fail.
    pub fn new(
        root: NativeSourceHandle<'a>,
        mappings: &[NativeLinkMapping<'a>],
    ) -> Result<Self, LinkSourceError> {
        let _span = tracing::debug_span!("native.linked.source.configure").entered();
        root.check()?;
        let pool = root
            .shared_budget()
            .ok_or(LinkSourceError::SharedBudgetRequired)?;
        charge(root, pool, mappings.len(), size_of::<Route<'_>>())?;
        let mut routes: Vec<Route<'a>> = Vec::new();
        routes
            .try_reserve_exact(mappings.len())
            .map_err(|_| QueryError::Allocation)?;

        for (index, mapping) in mappings.iter().enumerate() {
            charge(root, pool, 1, 0)?;
            for previous in &mappings[..index] {
                charge(root, pool, 1, 0)?;
                if previous.alias.eq_ignore_ascii_case(mapping.alias) {
                    return Err(LinkSourceError::DuplicateMapping);
                }
            }
            validate_handle(root, mapping.target, &routes)?;
            let local = match root.table_identity(mapping.alias) {
                Err(QueryError::UnknownName) => return Err(LinkSourceError::ExtraMapping),
                result => result?,
            };
            if local.storage() == NativeTableStorage::Local {
                return Err(LinkSourceError::ExtraMapping);
            }
            if local.name() != mapping.alias || local.native_id() != mapping.native_id {
                return Err(LinkSourceError::IdentityMismatch);
            }
            let definition = root.read_linked_table_definition(local.name())?;
            if definition.object_id() != local.object_id()
                || definition.native_id() != local.native_id()
                || definition.name() != local.name()
                || definition.storage() != local.storage()
            {
                return Err(LinkSourceError::IdentityMismatch);
            }
            let target = definition.access_target().map_err(|error| match error {
                InspectError::Unsupported => LinkSourceError::UnsupportedTarget,
                other => LinkSourceError::Native(other),
            })?;
            if target.database() != mapping.database || target.table() != mapping.target_table {
                return Err(LinkSourceError::IdentityMismatch);
            }
            let remote = mapping.target.table_identity(mapping.target_table)?;
            if remote.storage() != NativeTableStorage::Local {
                return Err(LinkSourceError::UnsupportedTarget);
            }
            if remote.native_id() != mapping.target_native_id {
                return Err(LinkSourceError::IdentityMismatch);
            }
            // Scalar schema discovery is metadata-only and uses the same native
            // pool. Collection policies remain the target's explicitly chosen view.
            mapping.target.source().columns(remote.name())?;
            routes.push(Route {
                alias: local.name(),
                target: mapping.target,
                table: remote.name(),
            });
        }

        root.check()?;
        Ok(Self { root, routes })
    }

    pub(super) fn root_handle(&self) -> NativeSourceHandle<'a> {
        self.root
    }

    pub(super) fn validate_external_target(
        &self,
        target: NativeSourceHandle<'a>,
    ) -> Result<(), LinkSourceError> {
        if self.root.same_source(target) {
            self.root.check()?;
            target.check()?;
            return Ok(());
        }

        validate_handle(self.root, target, &self.routes)
    }

    fn route(&self, table: &str) -> Result<(&'a dyn Source, &'a str), QueryError> {
        self.root.check()?;
        for route in &self.routes {
            self.root.check()?;
            self.root
                .shared_budget()
                .ok_or(QueryError::Source)?
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?
                .charge(Resource::Steps, 1)?;
            if route.alias.eq_ignore_ascii_case(table) {
                return Ok((route.target.source(), route.table));
            }
        }
        match self.root.table_identity(table) {
            Ok(identity) if identity.storage() != NativeTableStorage::Local => {
                Err(QueryError::Unsupported)
            }
            Ok(identity) => Ok((self.root.source(), identity.name())),
            Err(error) => Err(error),
        }
    }
}

pub(super) fn validate_external_handles(
    root: NativeSourceHandle<'_>,
    target: NativeSourceHandle<'_>,
) -> Result<(), LinkSourceError> {
    validate_handle(root, target, &[])
}

fn validate_handle(
    root: NativeSourceHandle<'_>,
    target: NativeSourceHandle<'_>,
    routes: &[Route<'_>],
) -> Result<(), LinkSourceError> {
    root.check()?;
    target.check()?;
    let root_pool = root
        .shared_budget()
        .ok_or(LinkSourceError::SharedBudgetRequired)?;
    if !target
        .shared_budget()
        .is_some_and(|target_pool| std::ptr::eq(root_pool, target_pool))
    {
        return Err(LinkSourceError::SharedBudgetRequired);
    }
    if root.scope() == target.scope() || root.same_source(target) {
        return Err(LinkSourceError::ScopeCollision);
    }
    for route in routes {
        charge(root, root_pool, 1, 0)?;
        if route.target.scope() == target.scope() && !route.target.same_source(target) {
            return Err(LinkSourceError::ScopeCollision);
        }
    }
    Ok(())
}

fn charge(
    root: NativeSourceHandle<'_>,
    pool: &RefCell<ResourceBudget>,
    count: usize,
    width: usize,
) -> Result<(), LinkSourceError> {
    root.check()?;
    let mut budget = pool
        .try_borrow_mut()
        .map_err(|_| InspectError::Unsupported)?;
    if width == 0 {
        budget
            .charge(Resource::Steps, count as u64)
            .map_err(InspectError::from)?;
    } else {
        budget
            .charge_allocation(count as u64, width as u64)
            .map_err(InspectError::from)?;
    }
    Ok(())
}
