//! Host-owned opening and policy setup shared by explicit native query targets.
use super::{Error, native_options, retrieval::NativeInput};
use crate::native;
use std::{cell::RefCell, path::Path};
use tablecraft_access::NativeSource;
use tablecraft_ace::{
    LongTextReadLimits, LongValueReadLimits, LongValueReadSource, TableSnapshotLimits,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub(super) type TargetSource<'a> = NativeSource<'a, LongValueReadSource<native::NativeFile>>;

pub(super) fn open<'a>(
    path: &Path,
    scope: CatalogScope,
    input: &NativeInput<'_>,
    pool: &'a RefCell<ResourceBudget>,
    comparison_pool: &'a RefCell<ResourceBudget>,
    database_policy: bool,
    snapshot_limits: TableSnapshotLimits,
) -> Result<TargetSource<'a>, Error> {
    let file = {
        let mut budget = pool.try_borrow_mut().map_err(|_| Error::Parameter)?;
        native::open(path, input.password, &mut budget).map_err(Error::Input)?
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
        scope,
        pool,
        &|| false,
        snapshot_limits,
    )
    .map_err(Error::Native)?;
    let source = native_options::configure(source, input, value_limits)?;

    if database_policy {
        let (source, _) = source
            .with_database_collation_resolver(comparison_pool)
            .map_err(Error::Native)?;
        Ok(source)
    } else {
        Ok(source)
    }
}
