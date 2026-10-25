//! Explicit CLI paths authorize exact IN tokens; stored tokens never open files.
use super::{
    Error,
    native_target::{self, TargetSource},
    retrieval::NativeInput,
};
use std::{cell::RefCell, path::Path};
use tablecraft_access::NativeExternalMapping;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, source::SourceKey};

pub(super) const MAPPING_FIELDS: usize = 2;
pub(super) const INDEX_FIELDS: usize = 3;
const FIRST_TARGET_SCOPE: u64 = 2;

struct Target<'a> {
    token: &'a str,
    key: SourceKey,
    source: TargetSource<'a>,
}

pub(super) struct Targets<'a> {
    sources: Vec<Target<'a>>,
}

impl Targets<'_> {
    pub(super) fn mappings<'a>(
        &'a self,
        pool: &RefCell<ResourceBudget>,
    ) -> Result<Vec<NativeExternalMapping<'a>>, Error> {
        charge::<NativeExternalMapping<'_>>(pool, self.sources.len())?;
        let mut mappings = Vec::new();
        mappings
            .try_reserve_exact(self.sources.len())
            .map_err(|_| Error::Retrieval(QueryError::Allocation))?;

        for target in &self.sources {
            mappings.push(NativeExternalMapping::new(
                target.token,
                target.key,
                target.source.handle(),
            ));
        }
        Ok(mappings)
    }
}

pub(super) fn load<'a>(
    input: &NativeInput<'a>,
    pool: &'a RefCell<ResourceBudget>,
    comparison_pool: &'a RefCell<ResourceBudget>,
    database_policy: bool,
    snapshot_limits: TableSnapshotLimits,
) -> Result<Targets<'a>, Error> {
    let (pairs, remainder) = input.external.as_chunks::<MAPPING_FIELDS>();
    if !remainder.is_empty() {
        return Err(Error::Parameter);
    }
    let selected_index = match input.external_index {
        [] => None,
        [token, table, index] => Some((token.as_str(), table.as_str(), index.as_str())),
        _ => return Err(Error::Parameter),
    };

    // Reject ambiguous tokens and unbound index requests before opening paths or
    // prompting for credentials. Comparisons consume the shared native allowance.
    let mut index_bound = selected_index.is_none();
    for (position, pair) in pairs.iter().enumerate() {
        let token = pair[0].to_str().ok_or(Error::Parameter)?;
        if let Some((selected, _, _)) = selected_index {
            compare_work(pool, selected, token)?;
            if selected == token {
                index_bound = true;
            }
        }
        for previous in &pairs[..position] {
            let previous = previous[0].to_str().ok_or(Error::Parameter)?;
            compare_work(pool, previous, token)?;
            if previous == token {
                return Err(Error::Parameter);
            }
        }
    }
    if !index_bound {
        return Err(Error::Parameter);
    }

    charge::<Target<'_>>(pool, pairs.len())?;
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(pairs.len())
        .map_err(|_| Error::Retrieval(QueryError::Allocation))?;
    let link_count = input.links.len() / super::links::MAPPING_FIELDS;
    let first_scope = u64::try_from(link_count)
        .ok()
        .and_then(|count| count.checked_add(FIRST_TARGET_SCOPE))
        .ok_or(Error::Parameter)?;

    for (position, pair) in pairs.iter().enumerate() {
        let token = pair[0].to_str().ok_or(Error::Parameter)?;
        let scope = u64::try_from(position)
            .ok()
            .and_then(|position| first_scope.checked_add(position))
            .ok_or(Error::Parameter)?;
        let source = native_target::open(
            Path::new(&pair[1]),
            CatalogScope::new(scope),
            input,
            pool,
            comparison_pool,
            database_policy,
            snapshot_limits,
        )?;
        if let Some((selected, _, _)) = selected_index {
            compare_work(pool, selected, token)?;
        }
        let source = match selected_index {
            Some((selected, table, index)) if selected == token => {
                source.with_index(table, index).map_err(Error::Native)?
            }
            _ => source,
        };
        sources.push(Target {
            token,
            key: SourceKey::new(u128::from(scope)),
            source,
        });
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

fn compare_work(pool: &RefCell<ResourceBudget>, left: &str, right: &str) -> Result<(), Error> {
    let work = left
        .len()
        .checked_add(right.len())
        .and_then(|length| length.checked_add(1))
        .ok_or(Error::Parameter)?;
    pool.try_borrow_mut()
        .map_err(|_| Error::Parameter)?
        .charge(Resource::Steps, work as u64)
        .map_err(|error| Error::Native(error.into()))
}
