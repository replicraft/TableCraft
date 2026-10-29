//! Capture every physical source owner before the first append reservation.
use crate::{
    EditError, IndexCheck, IndexCheckStatus, InspectError, NativeIndex, TableSchema, WriteBlocker,
    format::{allocation, bytes::u32_at, reader::Reader},
    table::indexes::mutation::tree::Tree,
};
use std::io::Cursor;

pub(super) struct Physical<'a> {
    pub index: &'a NativeIndex,
    pub pages: Vec<u32>,
}

pub(super) fn capture<'a>(
    schema: &TableSchema,
    indexes: &'a [IndexCheck],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Physical<'a>>, EditError> {
    let mut result: Vec<Physical<'a>> = Vec::new();
    super::reserve(&mut result, schema.layout.physical_indexes, reader)?;
    for physical in 0..schema.layout.physical_indexes {
        reader.tick()?;
        let number = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
        reader.work(indexes.len())?;
        let index = indexes
            .iter()
            .find(|index| index.index().data_number() == number)
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        for alias in indexes
            .iter()
            .filter(|alias| alias.index().data_number() == number)
        {
            reader.tick()?;
            if alias.status() != IndexCheckStatus::Checked
                || alias.index().root_page() != index.index().root_page()
                || alias.index().usage_map() != index.index().usage_map()
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        let index = index.index();
        let pages = allocation::pages(reader, index.usage_map())?;
        for pair in pages.windows(2) {
            reader.tick()?;
            if pair[0] >= pair[1] {
                return Err(InspectError::Malformed.into());
            }
        }
        if pages.binary_search(&index.root_page()).is_err() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        for &number in &pages {
            let page = reader.page(number)?;
            if !matches!(page[0], 3 | 4)
                || page[1] != 1
                || u32_at(&page, 4)? != schema.definition_page()
            {
                return Err(InspectError::Malformed.into());
            }
            for prior in &result {
                reader.tick()?;
                reader.work(prior.pages.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
                if prior.pages.binary_search(&number).is_ok() {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
        }
        result.push(Physical { index, pages });
    }
    reader.work(indexes.len())?;
    if indexes
        .iter()
        .any(|index| index.index().data_number() as usize >= result.len())
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    Ok(result)
}

pub(super) fn require_tree(
    tree: &Tree,
    expected: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    fn visit(
        tree: &Tree,
        pages: &mut Vec<u32>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.tick()?;
        super::reserve(pages, 1, reader)?;
        pages.push(tree.number());
        for child in tree.children() {
            visit(child, pages, reader)?;
        }
        Ok(())
    }
    let mut pages = Vec::new();
    visit(tree, &mut pages, reader)?;
    let levels = pages.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(
        pages
            .len()
            .saturating_mul(levels)
            .saturating_mul(size_of::<u32>()),
    )?;
    pages.sort_unstable();
    if pages != expected {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    Ok(())
}
