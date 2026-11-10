//! Chronological source proof only; current source/wire/Save routes remain closed.
#[cfg(test)]
mod tests;

use super::super::build;
use super::{
    FieldAdditionAfterOffError, FieldAdditionError, FieldAdditionTarget, kernel::Replay,
    source::ProvedImage,
};
use crate::{
    DatabaseNameAutoCorrect, FileFormat, InspectError, NativeImage,
    creation::{FieldHistoryOffSource, add_column::common},
    format::{
        page::PAGE_BYTES,
        reader::{Reader, row_count, row_range},
        row::{OFFSET_MASK, reference_page, reference_slot},
    },
    header::check,
    inspect_database_properties, inspect_table_schema,
    table::mutation::catalog_property::{self, column, descriptor, ledger},
};
use std::io::Cursor;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

use FieldAdditionAfterOffHistoryError as Error;

const SOURCE_PAGES: usize = 11;
const SOURCE_OBJECTS: usize = 9;
const PROPERTY_PAGE: u32 = 10;
const DATABASE_BODY_BYTES: usize = 152;

/// Refusal of the separate replay-based source or its consuming field addition.
#[derive(Debug, Error)]
pub enum FieldAdditionAfterOffHistoryError {
    /// Native read, cancellation, allocation or cumulative resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Actual settled proof, declaration, append or preservation failed.
    #[error(transparent)]
    Field(#[from] FieldAdditionAfterOffError),
    /// The original image is outside this bounded chronological source profile.
    #[error("source is outside the repeated ADD-after-OFF history profile")]
    Source,
    /// Full native replay did not reproduce the complete original source.
    #[error("repeated ADD-after-OFF source replay differs")]
    Preservation,
}

impl From<LimitError> for FieldAdditionAfterOffHistoryError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

impl NativeImage {
    /// Prove actual post-OFF snapshots by inverse hypothesis and complete native replay.
    /// This issues no mutation capability and does not widen any existing checker,
    /// candidate, source-purpose selector or TCAC request/recovery language.
    pub fn check_created_field_addition_after_off_history_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), FieldAdditionAfterOffHistoryError> {
        let _span = tracing::debug_span!("native.field_addition.post_off_history_source").entered();
        qualified_replay(self, table, budget, cancelled).map(drop)
    }
}

fn target(
    column: &crate::CreateColumn<'_>,
) -> Result<FieldAdditionTarget, FieldAdditionAfterOffHistoryError> {
    match FieldAdditionTarget::from_declaration(column) {
        Ok(target @ FieldAdditionTarget::NumberLongZero)
        | Ok(
            target @ FieldAdditionTarget::ShortText {
                max_utf16_units: super::TEXT_UNITS,
                unicode_compression: true,
            },
        ) => Ok(target),
        Ok(_) => Err(FieldAdditionAfterOffHistoryError::Source),
        Err(FieldAdditionError::Source) => Err(FieldAdditionAfterOffHistoryError::Source),
        Err(error) => Err(FieldAdditionAfterOffError::Field(error).into()),
    }
}

pub(super) fn qualified_replay(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Replay, FieldAdditionAfterOffHistoryError> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    if image.catalog().header().format() != FileFormat::Ace2016
        || image.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
        || image.catalog().origins().len() != SOURCE_OBJECTS
        || table.len() > super::MAX_ASCII_NAME_BYTES
    {
        return Err(Error::Source);
    }

    budget.charge(Resource::Steps, table.len() as u64)?;
    if !table.is_ascii() {
        return Err(Error::Source);
    }

    let (checked, definition) =
        super::super::super::source::inspect_field_addition(image, table, budget, cancelled)
            .map_err(FieldAdditionError::from)
            .map_err(FieldAdditionAfterOffError::from)?;
    if !checked.schema().name().is_ascii() {
        return Err(Error::Source);
    }
    for field in checked.schema().columns() {
        check(budget, cancelled)?;
        if !field.name().is_ascii() {
            return Err(Error::Source);
        }
    }

    let scope = image.catalog().catalog().scope();
    let database = inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?
        .ok_or(Error::Source)?;
    if database.raw_bytes().len() != DATABASE_BODY_BYTES
        || database.name_auto_correct(budget, cancelled)? != DatabaseNameAutoCorrect::ExplicitOff
    {
        return Err(Error::Source);
    }

    let live = ledger::capture(image.as_bytes(), image.catalog(), budget, cancelled)
        .map_err(FieldAdditionAfterOffError::from)?;
    let (user, user_bytes) = live
        .single_reference(checked.schema().object_id(), budget, cancelled)?
        .ok_or(Error::Source)?;
    let (db, db_bytes) = live
        .single_reference(database.object_id(), budget, cancelled)?
        .ok_or(Error::Source)?;
    if reference_page(user) != PROPERTY_PAGE
        || reference_page(db) != PROPERTY_PAGE
        || reference_slot(db) == 0
        || reference_slot(user) <= reference_slot(db)
        || db_bytes != DATABASE_BODY_BYTES
        || !live.two_singles(
            [
                (checked.schema().object_id(), user, user_bytes),
                (database.object_id(), db, db_bytes),
            ],
            budget,
            cancelled,
        )?
    {
        return Err(Error::Source);
    }
    let catalog =
        inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
    let selected = column(&catalog).map_err(FieldAdditionAfterOffError::from)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == checked.schema().object_id())
        .ok_or(Error::Source)?;

    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };

    let observed = descriptor::capture(&mut reader, &catalog, selected, origin)?;
    if !observed.canonical_single(user, user_bytes)? {
        return Err(Error::Source);
    }
    let descriptor = observed.absolute_range()?.ok_or(Error::Source)?;

    let page = reader.page(PROPERTY_PAGE)?;
    let count = row_count(&page)?;
    if count.checked_sub(1) != Some(usize::from(reference_slot(user))) {
        return Err(Error::Source);
    }
    for slot in 0..count {
        reader.tick()?;
        let (flags, start, end) = row_range(&page, slot)?;
        if flags & !OFFSET_MASK != 0 || start == end {
            return Err(Error::Source);
        }
    }

    // Only the actual independently owned DB and live user references delimit
    // post-DB snapshots. This is never total-LVAL-row-count-as-field-history.
    let events = usize::from(reference_slot(user) - reference_slot(db));
    let columns = definition
        .borrowed_columns(reader.budget, cancelled)
        .map_err(FieldAdditionError::from)
        .map_err(FieldAdditionAfterOffError::from)?;
    let prefix = columns
        .len()
        .checked_sub(events)
        .filter(|prefix| *prefix > 1)
        .ok_or(Error::Source)?;

    let order = build::initial_order(&columns[..prefix], reader.budget, cancelled)
        .map_err(FieldAdditionAfterOffError::from)?;
    let expected_prefix = build::canonical(
        checked.schema().name(),
        &columns[..prefix],
        order,
        reader.budget,
        cancelled,
    )
    .map_err(FieldAdditionAfterOffError::from)?;

    let mut hypothesis = common::copy_definition(
        image,
        &expected_prefix,
        checked.schema().definition_page() as usize * PAGE_BYTES,
        reader.budget,
        cancelled,
    )?;
    catalog_property::append_inverse::user_snapshots(
        &mut hypothesis,
        descriptor,
        db,
        user,
        reader.budget,
        cancelled,
    )
    .map_err(FieldAdditionAfterOffError::from)?;

    let recovered = NativeImage::capture(
        &mut Cursor::new(hypothesis),
        scope,
        reader.budget,
        cancelled,
    )?;
    let proof = FieldHistoryOffSource::capture(recovered, reader.budget, cancelled)
        .map_err(FieldAdditionAfterOffError::from)?;

    let first = &columns[prefix];
    let mut replay = Replay::start(
        proof,
        table,
        first.name(),
        target(first)?,
        reader.budget,
        cancelled,
    )?;
    for field in &columns[prefix + 1..] {
        check(reader.budget, cancelled)?;
        replay = replay.advance(
            table,
            field.name(),
            target(field)?,
            reader.budget,
            cancelled,
        )?;
    }

    if !common::equal_image(
        image.as_bytes(),
        replay.image().as_bytes(),
        reader.budget,
        cancelled,
    )? {
        // A valid current file can retain bytes outside this chronological
        // producer language. Source replay is admission, not an AFTER proof.
        return Err(Error::Source);
    }
    check(reader.budget, cancelled)?;
    Ok(replay)
}
