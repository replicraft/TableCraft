//! Byte composition for an independently source-qualified creator LvProp ADD.
//! This module grants no catalog UPDATE admission. Its only consumer must first
//! prove the complete canonical creator image, including empty LVAL roles and
//! the last logical catalog root belonging to its selected single user table.
mod maps;
mod record;

use super::{
    EditError,
    record_placement::{audit, forward::Plan, space::Space},
    record_roots,
    record_serialization::long_value::{Encoded, StorageChoice},
    rows::RowAssignment,
};
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

struct Selected(usize);

impl RowAssignment for Selected {
    fn row(&self) -> usize {
        self.0
    }
}

pub(crate) fn compose(
    image: &NativeImage,
    table: &TableSchema,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Change, EditError> {
    let scope = image.catalog().catalog().scope();
    let catalog =
        inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == table.object_id())
        .ok_or(InspectError::Malformed)?;
    let ordinal = usize::try_from(catalog.declared_rows())
        .ok()
        .and_then(|count| count.checked_sub(1))
        .ok_or(InspectError::Malformed)?;
    let updates = [Selected(ordinal)];
    let roots = record_roots::capture(image, &catalog, &updates, budget, cancelled)?;
    let root = roots.selected.first().ok_or(EditError::Preservation)?;
    if roots.selected.len() != 1 || root.reference != reference(origin.page(), origin.slot()) {
        return Err(EditError::Preservation);
    }
    let column = catalog
        .columns()
        .iter()
        .position(|field| field.name() == "LvProp")
        .ok_or(InspectError::Malformed)?;
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
    let record = record::compose(
        &catalog,
        column,
        &root.final_record.data,
        &descriptor,
        budget,
        cancelled,
    )?;
    // Complete canonical source equality is this caller's exclusion proof. This
    // is deliberately not a retry after normal user-row admission has failed.
    let space = Space::capture(image, &catalog, &roots.owned, budget, cancelled)?;
    let mut placement = Plan::new();
    placement.apply(image, root, &record, &space, budget, cancelled)?;
    audit::pages(&placement, budget, cancelled)?;
    if placement.pages.len() != 1 {
        return Err(EditError::Preservation);
    }
    let patch = placement.pages.pop().ok_or(EditError::Preservation)?;
    let Encoded::Present(body) = encoded else {
        return Err(EditError::Preservation);
    };
    Ok(Change {
        page: patch.number,
        bytes: patch.after,
        maps,
        body,
    })
}
