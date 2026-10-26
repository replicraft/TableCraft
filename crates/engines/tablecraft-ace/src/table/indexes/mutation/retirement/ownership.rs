//! Cross-table claims use original map populations, never table-local index IDs.
use super::RetirementTable;
use crate::{
    EditError, InspectError, NativeImage,
    format::{allocation, reader::Reader},
    header,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn check(
    image: &NativeImage,
    tables: &[RetirementTable<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut claims: Vec<Vec<u32>> = Vec::new();
    for table in tables {
        let data = allocation::pages(&mut reader, table.schema.layout.usage)?;
        claim(&mut claims, data, tables, &mut reader)?;
        for (position, checked) in table.indexes.iter().enumerate() {
            reader.tick()?;
            reader.work(position)?;
            let index = checked.index();
            if table.indexes[..position]
                .iter()
                .any(|previous| previous.index().data_number() == index.data_number())
            {
                continue;
            }
            let pages = allocation::pages(&mut reader, index.usage_map())?;
            claim(&mut claims, pages, tables, &mut reader)?;
        }
    }
    header::check(reader.budget, reader.cancelled)?;
    Ok(())
}

fn claim(
    claims: &mut Vec<Vec<u32>>,
    pages: Vec<u32>,
    tables: &[RetirementTable<'_>],
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
) -> Result<(), EditError> {
    for &page in &pages {
        reader.tick()?;
        reader.work(tables.len())?;
        if tables.iter().any(|table| table.schema.definition == page) {
            return Err(EditError::Preservation);
        }
        for previous in claims.iter() {
            reader.work(previous.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
            if previous.binary_search(&page).is_ok() {
                return Err(EditError::Preservation);
            }
        }
    }
    reader
        .budget
        .charge_allocation(1, size_of::<Vec<u32>>() as u64)
        .map_err(InspectError::from)?;
    claims
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    claims.push(pages);
    Ok(())
}
