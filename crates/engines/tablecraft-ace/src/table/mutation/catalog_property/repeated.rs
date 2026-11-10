//! Repeated Number property mutation under a separately complete source proof.
use super::super::record_roots;
use super::super::record_serialization::long_value::{Encoded, StorageChoice};
use super::{EditError, catalog, column, parent};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::mutation::owned_lval::{
        maps::Maps,
        property::{self, PropertyAppend},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(crate) struct RepeatedChange {
    pub catalog_page: u32,
    pub catalog_bytes: Vec<u8>,
    pub body: Vec<u8>,
    append: PropertyAppend,
}

fn references(
    image: &NativeImage,
    catalog: &TableSchema,
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<crate::LongValueMapReferences, EditError> {
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    super::maps::references(&mut reader, catalog, column)
}

/// Actual owned-page ledger; observing it grants no mutation authority.
pub(crate) fn repeated_property_pages(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    let catalog = catalog(image, budget, cancelled)?;
    let column = column(&catalog)?;
    let references = references(image, &catalog, column, budget, cancelled)?;
    let Maps { long_value, .. } =
        property::capture_maps(image, &catalog, references, budget, cancelled)?;
    Ok(long_value.pages)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Parent {
    LastLogical,
    ActualOrigin,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SingleStorage {
    Preferred,
    Retained,
}

pub(crate) fn compose_repeated(
    image: &NativeImage,
    table: &TableSchema,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RepeatedChange, EditError> {
    compose(
        image,
        table,
        body,
        Parent::LastLogical,
        SingleStorage::Preferred,
        budget,
        cancelled,
    )
}

pub(crate) fn compose_fitted_at_origin(
    image: &NativeImage,
    table: &TableSchema,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RepeatedChange, EditError> {
    compose(
        image,
        table,
        body,
        Parent::ActualOrigin,
        SingleStorage::Preferred,
        budget,
        cancelled,
    )
}

/// Preserve an independently qualified original external Single property role.
/// The checked allocator/descriptor still validates the complete new body; a
/// shorter retained dictionary need not change its parent's storage convention.
pub(crate) fn compose_retained_single_at_origin(
    image: &NativeImage,
    table: &TableSchema,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RepeatedChange, EditError> {
    compose(
        image,
        table,
        body,
        Parent::ActualOrigin,
        SingleStorage::Retained,
        budget,
        cancelled,
    )
}

fn compose(
    image: &NativeImage,
    table: &TableSchema,
    body: Vec<u8>,
    selection: Parent,
    single: SingleStorage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RepeatedChange, EditError> {
    let catalog = catalog(image, budget, cancelled)?;
    let place = |root: &record_roots::Root<'_, parent::Selected>,
                 owned: &[u32],
                 budget: &mut ResourceBudget| {
        let column = column(&catalog)?;
        let references = references(image, &catalog, column, budget, cancelled)?;
        let append = match selection {
            Parent::LastLogical => {
                property::append(image, &catalog, references, &body, budget, cancelled)?
            }
            Parent::ActualOrigin => {
                property::append_fitted(image, &catalog, references, &body, budget, cancelled)?
            }
        };
        let encoded = Encoded::Present(body);
        if single == SingleStorage::Preferred && encoded.preferred() != StorageChoice::Single {
            return Err(InspectError::Unsupported.into());
        }
        let descriptor = encoded.descriptor(
            StorageChoice::Single,
            Some(append.reference),
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
        Ok(RepeatedChange {
            catalog_page,
            catalog_bytes,
            body,
            append,
        })
    };
    match selection {
        Parent::LastLogical => {
            parent::with_selected(image, table, &catalog, budget, cancelled, place)
        }
        Parent::ActualOrigin => {
            parent::with_origin(image, table, &catalog, budget, cancelled, place)
        }
    }
}

impl RepeatedChange {
    pub(crate) fn property_reference(&self) -> u32 {
        self.append.reference
    }

    pub(crate) fn length(&self) -> usize {
        self.append.length
    }

    pub(crate) fn lval_page(&self) -> u32 {
        self.append.page
    }

    pub(crate) fn map_byte_matches(&self, at: usize, before: u8, after: u8) -> bool {
        self.append
            .maps
            .iter()
            .any(|edit| edit.offset == at && edit.before == before && edit.after == after)
    }

    pub(crate) fn apply(
        &self,
        after: &mut Vec<u8>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        let extra = self
            .length()
            .checked_sub(after.len())
            .ok_or(EditError::Preservation)?;
        if extra != 0 && extra != PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        budget
            .charge_allocation(extra as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, extra as u64)
            .map_err(InspectError::from)?;
        after
            .try_reserve_exact(extra)
            .map_err(|_| InspectError::Allocation)?;
        after.resize(self.length(), 0);
        for (number, page) in [
            (self.catalog_page, &self.catalog_bytes),
            (self.append.page, &self.append.bytes),
        ] {
            check(budget, cancelled)?;
            let start = number as usize * PAGE_BYTES;
            after
                .get_mut(start..start + PAGE_BYTES)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(page);
        }
        for edit in &self.append.maps {
            check(budget, cancelled)?;
            let byte = after.get_mut(edit.offset).ok_or(EditError::Preservation)?;
            if *byte != edit.before {
                return Err(EditError::Preservation);
            }
            *byte = edit.after;
        }
        Ok(())
    }
}

/// Observe capacity from the actual column-owned maps and shared allocator rule.
pub(crate) fn fitted_property_capacity(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<usize>, EditError> {
    let catalog = catalog(image, budget, cancelled)?;
    let selected = column(&catalog)?;
    let references = references(image, &catalog, selected, budget, cancelled)?;
    property::fitted_capacity(image, &catalog, references, budget, cancelled)
}
