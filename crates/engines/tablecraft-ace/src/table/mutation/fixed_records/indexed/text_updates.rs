//! Fresh closed fitting Text UPDATE facts; cardinality-action callers stay separate.
use super::CheckedIndexes;
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, WriteBlocker,
    format::{
        allocation,
        data_type::{INT32, MAX_SHORT_TEXT_UNITS, SHORT_TEXT},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::{
        indexes::{
            check_table_retained,
            header::IndexHeader,
            page::{LEAF, PAGE_VERSION},
        },
        mutation::lval_ownership::Inventory,
    },
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

impl<'image> CheckedIndexes<'image> {
    pub(in crate::table::mutation) fn capture_text_updates(
        image: &'image NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let checked = check_table_retained(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let schema = checked.schema();
        let mut text = false;
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let live = Inventory::capture(image, reader.budget, cancelled)?;
        for selected in checked.indexes() {
            reader.tick()?;
            let index = selected.index();
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            reader.work(schema.columns().len())?;
            let (position, field) = schema
                .columns()
                .iter()
                .enumerate()
                .find(|(_, field)| field.number() == key.number())
                .ok_or(EditError::Preservation)?;
            let flags = index.constraints().ok_or(InspectError::Unsupported)?;
            if selected.status() != IndexCheckStatus::Checked
                || index.native_type() > 1
                || index.foreign_reference().is_some()
                || flags.ignore_nulls()
                || field.number() != position
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            match (field.native_type(), field.scalar_type()) {
                (INT32, Some(ScalarType::Int32))
                    if field.fixed()
                        && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                        && field.extended_flags() == 0
                        && field.storage_length() == size_of::<i32>() => {}
                (SHORT_TEXT, Some(ScalarType::Text))
                    if !field.fixed()
                        && field.flags() == UPDATABLE_FLAG
                        && field.extended_flags() == 0
                        && field.storage_length() > 0
                        && field.storage_length().is_multiple_of(size_of::<u16>())
                        && field.storage_length() / size_of::<u16>() <= MAX_SHORT_TEXT_UNITS
                        && field
                            .text_sort_order()
                            .and_then(|order| order.general_family())
                            .is_some()
                        && !flags.unique()
                        && !flags.required() =>
                {
                    text = true;
                }
                _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
            }
            let owned = allocation::pages(&mut reader, index.usage_map())?;
            reader.work(owned.len())?;
            if owned != [index.root_page()] {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            live.require_no_long_value_data_overlap(&owned, reader.budget, cancelled)?;
            let at = index.root_page() as usize * PAGE_BYTES;
            let page = image
                .as_bytes()
                .get(at..at + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            let header = IndexHeader::read(page)?;
            if header.kind != LEAF
                || header.version != PAGE_VERSION
                || header.owner.get() != schema.definition_page()
                || !header.qualified_for_update(0)
                || header.previous.get() != 0
                || header.next.get() != 0
                || header.tail.get() != 0
                || header.prefix.get() != 0
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        if !text {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        check(reader.budget, cancelled)?;
        Ok(Self { image, checked })
    }
}
