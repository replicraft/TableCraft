//! Complete original certificates and positive logical/physical target selection.
use super::super::{
    index_creation::{buffer, definition::DefinitionLayout},
    map_authority::{self, Owner},
    normal_ordinary, preserved_source,
};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, NativeIndex, WriteBlocker,
    format::{
        allocation::{self, MapRole, RepresentedMap},
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        reader::{Reader, row_count},
        row::{reference_page, reference_slot},
    },
    header::check,
    table::{indexes::check_table_retained, table_append::Inventory},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const ORDINARY_INDEX_FLAGS: u8 = 0x80;

pub(super) struct Original {
    pub content: preserved_source::Source,
    pub layout: DefinitionLayout,
    pub position: usize,
    pub remove_physical: bool,
    pub retired: Vec<u32>,
    pub native_table: i32,
}
impl Original {
    pub fn selected(&self) -> &NativeIndex {
        &self.content.indexes.indexes()[self.position]
    }
}

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Original, EditError> {
    check(budget, cancelled)?;
    image.require_retained_capture()?;
    Inventory::capture(image, budget, cancelled)?.require_source(image)?;
    if !crate::identifier::valid_name(requested, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    if !requested.is_ascii() {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let content = normal_ordinary::capture_retained_schema(image, table, budget, cancelled)?;
    if !content.dependencies.is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let schema = content.table.schema();
    let mut position = None;
    for (at, index) in content.indexes.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                index.name().len().max(requested.len()) as u64,
            )
            .map_err(InspectError::from)?;
        // Native Unicode identifier comparison is not established by SQL collation.
        if !index.name().is_ascii() {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        if index.name().eq_ignore_ascii_case(requested) && position.replace(at).is_some() {
            return Err(InspectError::Malformed.into());
        }
    }
    let position = position.ok_or(InspectError::UnknownIndex)?;
    let selected = &content.indexes.indexes()[position];
    let [key] = selected.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let column = schema
        .columns()
        .iter()
        .find(|field| field.number() == key.number())
        .ok_or(InspectError::Malformed)?;
    if selected.flags() != ORDINARY_INDEX_FLAGS
        || selected.native_type() != 0
        || selected.foreign_reference().is_some()
        || column.native_type() != INT32
        || column.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || column.extended_flags() != 0
        || column.storage_length() != size_of::<i32>()
        || column.scalar_type() != Some(tablecraft_schema::ScalarType::Int32)
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let checked = check_table_retained(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if checked
        .indexes()
        .iter()
        .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let layout = DefinitionLayout::read(image, schema, budget, cancelled)?;
    let remove_physical = !content
        .indexes
        .indexes()
        .iter()
        .enumerate()
        .any(|(at, index)| at != position && index.data_number() == selected.data_number());
    let mut retired = Vec::new();
    if remove_physical {
        let owner = Owner::IndexOwned {
            definition: schema.definition_page(),
            physical: selected.data_number(),
        };
        let census = map_authority::capture_catalog(image, budget, cancelled, |_, _, _, _| Ok(()))?;
        census.require_exclusive(&[owner], budget, cancelled)?;
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        // Unlike read_storage's observation-only footprint, this call checks
        // the selected owned-map row under its actual owner0 role before reuse.
        RepresentedMap::read(&mut reader, selected.usage_map(), MapRole::Owned)?;
        let storage =
            RepresentedMap::read_storage(&mut reader, selected.usage_map(), MapRole::Owned)?;
        // The complete owner census forbids overlapping records and shared bitmap
        // pages. A one-row owner0 host establishes whole-container liveness here.
        if reference_slot(selected.usage_map()) != 0
            || row_count(&reader.page(reference_page(selected.usage_map()))?)? != 1
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let members = allocation::pages(&mut reader, selected.usage_map())?;
        if members.binary_search(&selected.root_page()).is_err() {
            return Err(InspectError::Malformed.into());
        }
        let count = members
            .len()
            .checked_add(storage.pages.len())
            .ok_or(InspectError::StructureLimit)?;
        retired = buffer::vector(count, reader.budget, reader.cancelled)?;
        retired.extend(members);
        retired.extend(storage.pages);
        super::retirement::sort_pages(&mut retired, reader.budget, reader.cancelled)?;
        for pair in retired.windows(2) {
            reader.tick()?;
            if pair[0] == pair[1] {
                return Err(InspectError::Malformed.into());
            }
        }
    }
    let mut native_table = None;
    for origin in image.catalog().origins() {
        check(budget, cancelled)?;
        if origin.object_id() == schema.object_id()
            && native_table.replace(origin.native_id()).is_some()
        {
            return Err(InspectError::Malformed.into());
        }
    }
    Ok(Original {
        content,
        layout,
        position,
        remove_physical,
        retired,
        native_table: native_table.ok_or(InspectError::Malformed)?,
    })
}
