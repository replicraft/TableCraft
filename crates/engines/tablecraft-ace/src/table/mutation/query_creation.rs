//! Source-qualified new native query object; no ordinary system-row INSERT.
mod append;
mod initialize;
mod proof;
mod record;
mod source;

use crate::{
    NativeImage, NativeImageSource, NativeQueryStorageChange, QueryCreationDefinition,
    QueryCreationError, format::data_append::PageError, header::check,
};
use tablecraft_core::limits::ResourceBudget;

pub(crate) struct Built {
    pub after: NativeImage,
    pub native_id: i32,
    pub storage_change: NativeQueryStorageChange,
}

impl From<PageError> for QueryCreationError {
    fn from(error: PageError) -> Self {
        match error {
            PageError::Inspect(error) => Self::Inspect(error),
            PageError::Source | PageError::Capacity => Self::Source,
        }
    }
}

pub(crate) fn build(
    image: &NativeImage,
    definition: &QueryCreationDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Built, QueryCreationError> {
    check(budget, cancelled)?;
    if source::missing_queries(image, budget, cancelled)? {
        let initialized = initialize::build(image, definition, budget, cancelled)?;
        let mut built = build_existing(&initialized, definition, budget, cancelled)?;
        let definition_page = image.catalog().header().page_count();
        built.storage_change = NativeQueryStorageChange::Initialized {
            native_table_id: i32::try_from(definition_page)
                .map_err(|_| QueryCreationError::Source)?,
            definition_page,
        };
        return Ok(built);
    }
    build_existing(image, definition, budget, cancelled)
}

fn build_existing(
    image: &NativeImage,
    definition: &QueryCreationDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Built, QueryCreationError> {
    let source = source::capture(image, definition, budget, cancelled)?;
    let catalog = record::catalog(
        &source.storage.catalog,
        source.storage.parent,
        source.native_id,
        definition,
        budget,
        cancelled,
    )?;
    let fragments = record::fragments(
        &source.storage.queries,
        source.native_id,
        definition,
        budget,
        cancelled,
    )?;
    let bytes = append::build(
        image,
        &source.storage,
        &catalog,
        &fragments,
        budget,
        cancelled,
    )?;
    drop(catalog);
    drop(fragments);

    let owner = NativeImageSource::from_vec(bytes, budget, cancelled)?;
    let after = NativeImage::capture_shared_retained(
        owner,
        image.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    proof::verify(image, &after, &source, definition, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(Built {
        after,
        native_id: source.native_id,
        storage_change: NativeQueryStorageChange::Existing,
    })
}
