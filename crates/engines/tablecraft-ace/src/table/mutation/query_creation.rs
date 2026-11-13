//! Source-qualified new native query object; no ordinary system-row INSERT.
mod append;
mod proof;
mod record;
mod source;

use crate::{
    NativeImage, NativeImageSource, QueryCreationDefinition, QueryCreationError,
    format::data_append::PageError, header::check,
};
use tablecraft_core::limits::ResourceBudget;

pub(crate) struct Built {
    pub after: NativeImage,
    pub native_id: i32,
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
    })
}
