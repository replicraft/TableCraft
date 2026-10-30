//! Complete original metadata footprints deny DATA writes, not map-cell edits.
use super::super::{EditError, WriteBlocker, map_authority};
use crate::{
    IndexCheck, InspectError, NativeImage, NativeIndex, NativeReadSource, TableSchema,
    format::{allocation, definition, reader::Reader},
    header::check,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
mod tests;

pub(in crate::table::mutation) struct Metadata {
    // Operation-local identity of the immutable source buffer; never dereferenced.
    source_address: usize,
    source_length: usize,
    definition: u32,
    protected_pages: Vec<u32>,
    census: map_authority::Census,
    mutable: Vec<map_authority::Owner>,
}

#[derive(Clone, Copy)]
enum Capture<'a> {
    Column(usize),
    AllIndexes,
    Initialization(&'a [usize]),
    Mixed(&'a [usize]),
}

impl Metadata {
    pub(in crate::table::mutation) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        indexes: &[IndexCheck],
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_selected(
            image,
            schema,
            indexes,
            Capture::Column(column),
            budget,
            cancelled,
        )
    }

    /// INSERT maintains every physical index. It must capture all mutable index
    /// map roles, rather than borrow the authority of one updated field.
    pub(in crate::table::mutation) fn capture_for_insert(
        image: &NativeImage,
        schema: &TableSchema,
        indexes: &[IndexCheck],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_selected(
            image,
            schema,
            indexes,
            Capture::AllIndexes,
            budget,
            cancelled,
        )
    }

    pub(in crate::table::mutation) fn capture_for_insert_initialization(
        image: &NativeImage,
        schema: &TableSchema,
        indexes: &[IndexCheck],
        initializing: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        if initializing.is_empty() {
            return Self::capture_for_insert(image, schema, indexes, budget, cancelled);
        }

        Self::capture_selected(
            image,
            schema,
            indexes,
            Capture::Initialization(initializing),
            budget,
            cancelled,
        )
    }

    /// Capture the affected map-role union and every source body which may not
    /// receive an ordinary DATA record. The proof reacquires this independently.
    pub(in crate::table::mutation) fn capture_for_mixed_updates(
        image: &NativeImage,
        schema: &TableSchema,
        indexes: &[IndexCheck],
        columns: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        for &column in columns {
            check(budget, cancelled)?;
            schema
                .columns()
                .get(column)
                .ok_or(EditError::Preservation)?;
        }
        Self::capture_selected(
            image,
            schema,
            indexes,
            Capture::Mixed(columns),
            budget,
            cancelled,
        )
    }

    fn capture_selected(
        image: &NativeImage,
        schema: &TableSchema,
        indexes: &[IndexCheck],
        capture: Capture<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let column = match capture {
            Capture::Column(column) => Some(column),
            _ => None,
        };
        let initializing = match capture {
            Capture::Initialization(columns) => columns,
            _ => &[],
        };
        let number = column
            .map(|position| {
                schema
                    .columns()
                    .get(position)
                    .map(|column| column.number())
                    .ok_or(EditError::Unsupported(WriteBlocker::Column))
            })
            .transpose()?;
        let definition = schema.definition_page();
        let shared = [
            map_authority::Owner::GlobalFree,
            map_authority::Owner::OrdinaryOwned { definition },
            map_authority::Owner::OrdinaryCandidate { definition },
        ];
        let capacity = indexes
            .len()
            .checked_add(shared.len())
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge_allocation(capacity as u64, size_of::<map_authority::Owner>() as u64)
            .map_err(InspectError::from)?;
        let mut mutable = Vec::new();
        mutable
            .try_reserve_exact(capacity)
            .map_err(|_| InspectError::Allocation)?;
        mutable.extend_from_slice(&shared);
        let mut protected_pages = Vec::new();
        let mut selected_found = false;
        let mut payload = |local: &TableSchema,
                           native_indexes: &[NativeIndex],
                           declarations: &crate::table::long_value::maps::Declarations,
                           reader: &mut Reader<'_, Cursor<&[u8]>>|
         -> Result<(), EditError> {
            definition::read_with_pages(reader, local.definition_page(), |page, reader| {
                protect(&mut protected_pages, page, reader)
            })?;
            if matches!(capture, Capture::Mixed(_)) {
                if local.definition_page() != definition {
                    protect_members(&mut protected_pages, local.layout.usage, reader)?;
                }
                for index in native_indexes {
                    protect_members(&mut protected_pages, index.usage_map(), reader)?;
                }
                for declaration in &declarations.entries {
                    protect_members(&mut protected_pages, declaration.references.owned(), reader)?;
                }
            }
            if local.definition_page() != definition {
                return Ok(());
            }
            selected_found = true;
            reader.work(local.columns().len())?;
            if number
                .is_some_and(|number| !local.columns().iter().any(|field| field.number() == number))
                || native_indexes.len() != indexes.len()
            {
                return Err(EditError::Preservation);
            }
            for native in native_indexes {
                reader.tick()?;
                reader.work(indexes.len() + native.columns().len())?;
                let checked = indexes
                    .iter()
                    .find(|checked| checked.index().number() == native.number())
                    .ok_or(EditError::Preservation)?;
                if !same_index(native, checked.index()) {
                    return Err(EditError::Preservation);
                }
                if number.is_some_and(|number| {
                    !native
                        .columns()
                        .iter()
                        .any(|field| field.number() == number)
                }) {
                    continue;
                }
                if let Capture::Mixed(columns) = capture {
                    reader.work(
                        columns
                            .len()
                            .checked_mul(native.columns().len())
                            .ok_or(InspectError::StructureLimit)?,
                    )?;
                    if !columns.iter().any(|&column| {
                        native
                            .columns()
                            .iter()
                            .any(|key| key.number() == schema.columns()[column].number())
                    }) {
                        continue;
                    }
                }
                let owner = map_authority::Owner::IndexOwned {
                    definition,
                    physical: native.data_number(),
                };
                reader.work(mutable.len())?;
                if !mutable.contains(&owner) {
                    mutable.push(owner);
                }
            }
            Ok(())
        };
        let census = if initializing.is_empty() {
            map_authority::capture_catalog(image, budget, cancelled, &mut payload)?
        } else {
            map_authority::capture_catalog_for_initialization(
                image,
                Some((definition, initializing)),
                budget,
                cancelled,
                &mut payload,
            )?
        };
        if !selected_found {
            return Err(EditError::Preservation);
        }
        census.require_exclusive(&mutable, budget, cancelled)?;
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        census.protect_storage(&mut protected_pages, &mut reader)?;
        Ok(Self {
            source_address: image.as_bytes().as_ptr() as usize,
            source_length: image.as_bytes().len(),
            definition,
            protected_pages,
            census,
            mutable,
        })
    }

    pub(in crate::table::mutation) fn require_source(
        &self,
        image: &NativeImage,
        definition: u32,
    ) -> Result<(), EditError> {
        if self.source_address != image.as_bytes().as_ptr() as usize
            || self.source_length != image.as_bytes().len()
            || self.definition != definition
        {
            return Err(EditError::Preservation);
        }

        Ok(())
    }

    pub(in crate::table::mutation) fn require_exclusive(
        &self,
        owners: &[map_authority::Owner],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for owner in owners {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, self.mutable.len() as u64)
                .map_err(InspectError::from)?;
            if !self.mutable.contains(owner) {
                return Err(EditError::Preservation);
            }
        }

        self.census.require_exclusive(owners, budget, cancelled)
    }

    /// Deny only original definition/map storage. This grants no DATA owner,
    /// current hint, allocation or geometry authority for old or staged pages.
    pub(in crate::table::mutation) fn require_data_page(
        &self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for &page in &self.protected_pages {
            check(budget, cancelled)?;
            if number == page {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        check(budget, cancelled)?;
        Ok(())
    }

    /// Bind a rewritten original map row to this operation's captured roles.
    pub(in crate::table::mutation) fn require_record_rewrite(
        &self,
        owner: map_authority::Owner,
        reference: u32,
        range: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.census.require_record_rewrite(
            &self.mutable,
            map_authority::RecordIdentity {
                owner,
                reference,
                range,
            },
            budget,
            cancelled,
        )
    }
}

fn same_index(left: &NativeIndex, right: &NativeIndex) -> bool {
    left.number() == right.number()
        && left.data_number() == right.data_number()
        && left.root_page() == right.root_page()
        && left.usage_map() == right.usage_map()
        && left.native_type() == right.native_type()
        && left.flags() == right.flags()
        && left.columns() == right.columns()
}

fn protect<R: NativeReadSource>(
    pages: &mut Vec<u32>,
    number: u32,
    reader: &mut Reader<'_, R>,
) -> Result<(), InspectError> {
    reader.tick()?;
    reader.work(pages.len())?;
    if pages.contains(&number) {
        return Ok(());
    }
    reader
        .budget
        .charge_allocation(1, size_of::<u32>() as u64)?;
    pages.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    pages.push(number);
    Ok(())
}

fn protect_members<R: NativeReadSource>(
    protected: &mut Vec<u32>,
    reference: u32,
    reader: &mut Reader<'_, R>,
) -> Result<(), InspectError> {
    for page in allocation::pages(reader, reference)? {
        protect(protected, page, reader)?;
    }
    Ok(())
}
