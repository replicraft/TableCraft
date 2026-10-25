//! Explicit CLI target opening; the adapter owns native link validation/routing.
use super::{Error, native_options, retrieval::NativeInput};
use crate::native;
use std::{cell::RefCell, path::Path};
use tablecraft_access::{NativeLinkMapping, NativeSource, NativeSourceHandle};
use tablecraft_ace::{
    LongTextReadLimits, LongValueReadLimits, LongValueReadSource, NativeLinkedTableDefinition,
    NativeReadSource, TableSnapshotLimits,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_query::QueryError;

pub(super) const MAPPING_FIELDS: usize = 2;
const FIRST_LINK_SCOPE: u64 = 2;

type TargetSource<'a> = NativeSource<'a, LongValueReadSource<native::NativeFile>>;

struct Target<'a> {
    definition: NativeLinkedTableDefinition,
    source: TargetSource<'a>,
}

pub(super) struct Targets<'a> {
    sources: Vec<Target<'a>>,
}

impl Targets<'_> {
    pub(super) fn mappings<'a>(
        &'a self,
        root: NativeSourceHandle<'a>,
        pool: &RefCell<ResourceBudget>,
    ) -> Result<Vec<NativeLinkMapping<'a>>, Error> {
        charge::<NativeLinkMapping<'_>>(pool, self.sources.len())?;
        let mut mappings = Vec::new();
        mappings
            .try_reserve_exact(self.sources.len())
            .map_err(|_| Error::Retrieval(QueryError::Allocation))?;

        for target in &self.sources {
            let definition = &target.definition;
            let link = definition.access_target().map_err(Error::Native)?;
            let remote = target
                .source
                .handle()
                .table_identity(link.table())
                .map_err(Error::Retrieval)?;
            let local = root
                .table_identity(definition.name())
                .map_err(Error::Retrieval)?;
            mappings.push(NativeLinkMapping::new(
                local.name(),
                local.native_id(),
                link.database(),
                target.source.handle(),
                link.table(),
                remote.native_id(),
            ));
        }
        Ok(mappings)
    }
}

pub(super) fn load<'a, R: NativeReadSource>(
    root: &NativeSource<'_, R>,
    input: &NativeInput<'_>,
    pool: &'a RefCell<ResourceBudget>,
    comparison_pool: &'a RefCell<ResourceBudget>,
    database_policy: bool,
    snapshot_limits: TableSnapshotLimits,
) -> Result<Targets<'a>, Error> {
    let (pairs, remainder) = input.links.as_chunks::<MAPPING_FIELDS>();
    if !remainder.is_empty() {
        return Err(Error::Parameter);
    }
    let count = pairs.len();
    charge::<Target<'_>>(pool, count)?;
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(count)
        .map_err(|_| Error::Retrieval(QueryError::Allocation))?;

    for (index, pair) in pairs.iter().enumerate() {
        let alias = pair[0].to_str().ok_or(Error::Parameter)?;
        let definition = root
            .handle()
            .read_linked_table_definition(alias)
            .map_err(Error::Native)?;
        // Validate the provider before opening the explicitly supplied path or
        // requesting a password. Stored Database is never a path to open here.
        definition.access_target().map_err(Error::Native)?;
        let scope = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(FIRST_LINK_SCOPE))
            .ok_or(Error::Parameter)?;
        let file = {
            let mut budget = pool.try_borrow_mut().map_err(|_| Error::Parameter)?;
            native::open(Path::new(&pair[1]), input.password, &mut budget).map_err(Error::Input)?
        };
        let value_limits = input
            .native_value_bytes
            .map(LongValueReadLimits::new)
            .transpose()
            .map_err(Error::Native)?
            .unwrap_or_default();
        let text_limits = input
            .native_text_bytes
            .map(LongTextReadLimits::new)
            .transpose()
            .map_err(Error::Native)?
            .unwrap_or_default();
        let file = LongValueReadSource::new(file, value_limits).with_text_limits(text_limits);
        let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
            file,
            CatalogScope::new(scope),
            pool,
            &|| false,
            snapshot_limits,
        )
        .map_err(Error::Native)?;
        let source = native_options::configure(source, input, value_limits)?;
        let source = if database_policy {
            let (source, _) = source
                .with_database_collation_resolver(comparison_pool)
                .map_err(Error::Native)?;
            source
        } else {
            source
        };
        let source = match input.index {
            [alias, index] if alias.eq_ignore_ascii_case(definition.name()) => {
                let link = definition.access_target().map_err(Error::Native)?;
                source
                    .with_index(link.table(), index)
                    .map_err(Error::Native)?
            }
            _ => source,
        };
        sources.push(Target { definition, source });
    }
    Ok(Targets { sources })
}

fn charge<T>(pool: &RefCell<ResourceBudget>, count: usize) -> Result<(), Error> {
    pool.try_borrow_mut()
        .map_err(|_| Error::Parameter)?
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map(|_| ())
        .map_err(|error| Error::Native(error.into()))
}
