//! Byte composition for an independently source-qualified creator LvProp ADD.
//! This module grants no catalog UPDATE admission. Each caller must first prove
//! its complete canonical source/history, including exact LVAL/map ownership and
//! the last logical catalog root belonging to its selected single user table.
mod existing;
mod maps;
mod parent;
mod record;
pub(crate) use existing::{ExistingChange, compose_existing};

use super::EditError;
use super::record_serialization::long_value::{Encoded, StorageChoice};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{allocation::MapCell, page::PAGE_BYTES, row::reference},
    inspect_table_schema,
};
use tablecraft_core::limits::ResourceBudget;

pub(crate) struct Change {
    pub page: u32,
    pub bytes: Vec<u8>,
    pub maps: [MapCell; 2],
    pub body: Vec<u8>,
}

pub(super) fn catalog(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSchema, EditError> {
    let scope = image.catalog().catalog().scope();
    Ok(inspect_table_schema(
        &mut image.reader(),
        scope,
        "MSysObjects",
        budget,
        cancelled,
    )?)
}

pub(super) fn column(catalog: &TableSchema) -> Result<usize, EditError> {
    catalog
        .columns()
        .iter()
        .position(|field| field.name() == "LvProp")
        .ok_or(InspectError::Malformed.into())
}

pub(crate) fn compose(
    image: &NativeImage,
    table: &TableSchema,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Change, EditError> {
    let catalog = catalog(image, budget, cancelled)?;
    parent::with_selected(
        image,
        table,
        &catalog,
        budget,
        cancelled,
        |root, owned, budget| {
            let column = column(&catalog)?;
            let new_page = u32::try_from(image.as_bytes().len() / PAGE_BYTES)
                .map_err(|_| InspectError::StructureLimit)?;
            let maps = maps::capture(image, &catalog, column, new_page, budget, cancelled)?;
            let encoded = Encoded::Present(body);
            if encoded.preferred() != StorageChoice::Single {
                return Err(InspectError::Unsupported.into());
            }
            let descriptor = encoded.descriptor(
                StorageChoice::Single,
                Some(reference(new_page, 0)),
                budget,
                cancelled,
            )?;
            let (page, bytes) = parent::place(
                image,
                &catalog,
                column,
                &descriptor,
                root,
                owned,
                budget,
                cancelled,
            )?;
            let Encoded::Present(body) = encoded else {
                return Err(EditError::Preservation);
            };
            Ok(Change {
                page,
                bytes,
                maps,
                body,
            })
        },
    )
}
