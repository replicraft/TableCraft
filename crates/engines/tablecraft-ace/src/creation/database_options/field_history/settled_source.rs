//! A recovered image is only a hypothesis until real OFF replay equals all bytes.
#[cfg(test)]
mod tests;

use super::{
    FieldHistoryDatabaseOptionsError as Error, PROPERTY_PAGE, SOURCE_OBJECTS, catalog_record_length,
};
use crate::{
    DatabaseNameAutoCorrect, FileFormat, InspectError, NativeImage,
    creation::{
        database_options::{DATABASE_ID, DATABASE_PARENT, properties},
        record::DATABASE_SYSTEM_FLAGS,
    },
    format::{
        page::PAGE_BYTES,
        reader::{Reader, row_count},
        row::{reference, reference_slot},
    },
    header::check,
    inspect_database_properties, inspect_table_schema,
    table::mutation::catalog_property::{append_inverse, column, descriptor},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG_PAGE: u32 = 4;
const DATABASE_TYPE: i16 = 2;
const SOURCE_PAGES: usize = 11;
const MIN_DATABASE_SLOT: u8 = 8;
const MAX_DATABASE_SLOT: u8 = 9;

impl NativeImage {
    /// Prove the complete current OFF-after-field-history source by real replay.
    ///
    /// The bounded inverse creates only an eight-role hypothesis. The actual
    /// OFF constructor must prove its full Source5 and reproduce EVERY current
    /// byte, including the database role, both live properties and retired rows.
    /// This separate inert source proof neither widens old source/request
    /// languages nor authorizes a rename, persistence or another field addition.
    pub fn check_field_history_name_auto_correct_off_source(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        validate(self, budget, cancelled)
    }
}

/// The original image is retained only after the complete current replay proof.
/// Private fields prevent a caller-supplied boolean or arbitrary image from
/// constructing a source qualification for another mutation.
pub(crate) struct FieldHistoryOffSource {
    image: NativeImage,
}

impl FieldHistoryOffSource {
    pub(crate) fn capture(
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        validate(&image, budget, cancelled)?;
        Ok(Self { image })
    }

    pub(crate) fn image(&self) -> &NativeImage {
        &self.image
    }
}

fn validate(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let _span = tracing::debug_span!("native.database_options.field_history_off_source").entered();
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    if image.catalog().header().format() != FileFormat::Ace2016
        || image.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
        || image.catalog().origins().len() != SOURCE_OBJECTS + 1
    {
        return Err(Error::Source);
    }
    let scope = image.catalog().catalog().scope();
    let options = inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?
        .ok_or(Error::Source)?;
    if options.raw_bytes().len() != properties::ENCODED_BYTES
        || options.name_auto_correct(budget, cancelled)? != DatabaseNameAutoCorrect::ExplicitOff
    {
        return Err(Error::Source);
    }
    let database = image.catalog().origins().last().ok_or(Error::Source)?;
    if database.object_id() != options.object_id()
        || database.native_id() != DATABASE_ID as i32
        || database.parent_id() != DATABASE_PARENT as i32
        || database.type_code() != DATABASE_TYPE
        || database.flags() != DATABASE_SYSTEM_FLAGS
        || database.page() != CATALOG_PAGE
        || !(MIN_DATABASE_SLOT..=MAX_DATABASE_SLOT).contains(&database.slot())
    {
        return Err(Error::Source);
    }
    let catalog =
        inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
    let property_column = column(&catalog)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let page = reader.page(PROPERTY_PAGE as u32)?;
    let count = row_count(&page)?;
    let last = count
        .checked_sub(1)
        .filter(|last| *last > 0)
        .ok_or(Error::Source)?;
    let property = reference(
        PROPERTY_PAGE as u32,
        u8::try_from(last).map_err(|_| Error::Source)?,
    );
    let observed = descriptor::capture(&mut reader, &catalog, property_column, database)?;
    if !observed.canonical_single(property, properties::ENCODED_BYTES)? {
        return Err(Error::Source);
    }
    let record_bytes = catalog_record_length()?;
    let hypothesis = append_inverse::hypothesis(
        image,
        append_inverse::LastRole {
            root: reference(CATALOG_PAGE, database.slot()),
            property,
            record_bytes,
            body_bytes: properties::ENCODED_BYTES,
        },
        reader.budget,
        cancelled,
    )?;
    let recovered = NativeImage::capture(
        &mut Cursor::new(hypothesis),
        scope,
        reader.budget,
        cancelled,
    )?;
    // This is the actual source-qualified candidate constructor. It performs
    // complete canonical seed/history equality, macro/role/ownership checks, allocator placement
    // and old/new live-role verification. No decoder/certificate substitutes.
    let replay = recovered
        .prepare_disable_name_auto_correct_after_field_history(reader.budget, cancelled)?;
    if replay.property_reference() != property
        || reference_slot(replay.property_reference()) == 0
        || replay.after_bytes().len() != image.as_bytes().len()
    {
        return Err(Error::Preservation);
    }
    for (actual, expected) in image
        .as_bytes()
        .chunks(PAGE_BYTES)
        .zip(replay.after_bytes().chunks(PAGE_BYTES))
    {
        check(reader.budget, cancelled)?;
        reader
            .budget
            .charge(Resource::Steps, (actual.len() + expected.len()) as u64)
            .map_err(InspectError::from)?;
        if actual != expected {
            return Err(Error::Preservation);
        }
    }
    check(reader.budget, cancelled)?;
    Ok(())
}
