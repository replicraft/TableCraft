//! Shared retained schema publication keeps each selected DROP language sealed.
use super::{
    FieldDropError as Error, definition,
    schema_source::{self, Selection, Source},
    schema_verify,
};
use crate::{
    NativeImage, format::page::PAGE_BYTES, header::check,
    table::mutation::catalog_property::retained_snapshot,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Candidate {
    pub source: Source,
    pub after: Vec<u8>,
}

pub(super) fn prepare(
    image: NativeImage,
    table: &str,
    column: &str,
    selection: Selection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Candidate, Error> {
    let _span = tracing::debug_span!("native.retained_field_drop.prepare").entered();
    if selection == Selection::Current {
        return Err(Error::Source);
    }
    let mut source = schema_source::capture(image, table, column, selection, budget, cancelled)?;
    let before = source.proof.image();
    let page = definition::page(before, &source.evidence, budget, cancelled)?;
    let mut after = definition::copy_image(before, budget, cancelled)?;
    let base = source.evidence.observed.schema.definition_page() as usize * PAGE_BYTES;
    after[base..base + page.len()].copy_from_slice(&page);
    let body = std::mem::take(&mut source.evidence.body);
    let snapshot =
        retained_snapshot::compose(before, source.proof.table(), body, budget, cancelled)?;
    snapshot.apply(&mut after, budget, cancelled)?;
    schema_verify::retained_candidate(&source, &after, &snapshot, budget, cancelled)?;
    schema_verify::survivors(&source, &after, budget, cancelled)?;
    drop(snapshot);
    check(budget, cancelled)?;
    Ok(Candidate { source, after })
}
