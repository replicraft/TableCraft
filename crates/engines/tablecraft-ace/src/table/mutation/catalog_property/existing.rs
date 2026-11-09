//! Existing owned LVAL append for a separately complete-source-qualified caller.
use super::{EditError, catalog, column, parent};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::reader::Reader,
    table::{
        long_value::maps::declarations,
        mutation::{
            owned_lval::placement::append_existing_lval,
            record_serialization::long_value::{Encoded, StorageChoice},
        },
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(crate) struct ExistingChange {
    pub catalog_page: u32,
    pub catalog_bytes: Vec<u8>,
    pub lval_page: u32,
    pub lval_bytes: Vec<u8>,
    pub body: Vec<u8>,
}

pub(crate) fn compose_existing(
    image: &NativeImage,
    table: &TableSchema,
    page: u32,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ExistingChange, EditError> {
    let catalog = catalog(image, budget, cancelled)?;
    parent::with_selected(
        image,
        table,
        &catalog,
        budget,
        cancelled,
        |root, owned, budget| {
            let column = column(&catalog)?;
            let mut source = image.reader();
            let mut reader = Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            };
            let declared = declarations(&mut reader, &catalog)?;
            let selected = declared
                .entries
                .iter()
                .find(|entry| entry.number == catalog.columns()[column].number())
                .ok_or(InspectError::Unsupported)?;
            if !declared.complete {
                return Err(InspectError::Unsupported.into());
            }
            let references = selected.references;
            let appended =
                append_existing_lval(image, &catalog, references, page, &body, budget, cancelled)?;
            let encoded = Encoded::Present(body);
            if encoded.preferred() != StorageChoice::Single {
                return Err(InspectError::Unsupported.into());
            }
            let descriptor = encoded.descriptor(
                StorageChoice::Single,
                Some(appended.reference),
                budget,
                cancelled,
            )?;
            let (catalog_page, catalog_bytes) = parent::place(
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
            Ok(ExistingChange {
                catalog_page,
                catalog_bytes,
                lval_page: appended.page,
                lval_bytes: appended.bytes,
                body,
            })
        },
    )
}
