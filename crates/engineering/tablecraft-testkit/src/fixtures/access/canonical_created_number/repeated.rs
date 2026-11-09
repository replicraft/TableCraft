//! Independently framed bounded Number property histories, without ACE codecs.
use super::{
    DIRECTORY, MASK_BYTES, PAGE, PROPERTY_VARIABLE, Source, append as first, number_group,
    source as initial, word,
};
use crate::fixtures::access::{canonical_created_add_column, put16, put32};

const CATALOG_PAGE: usize = 4;
const DEFINITION_PAGE: usize = 7;
const PROPERTY_PAGE: usize = 10;
const ORIGINAL_PAGE_COUNT: usize = PROPERTY_PAGE;
const LOGICAL_CATALOG_SLOT: usize = 7;
const FORWARD_TARGET_SLOT: usize = LOGICAL_CATALOG_SLOT + 1;
const ROW_COUNT_OFFSET: usize = 12;
const FREE_SPACE_OFFSET: usize = 2;
const FORWARD_FLAG: usize = 0x4000;
const OFFSET_MASK: usize = 0x1fff;
const SINGLE_PROPERTY_FLAG: u32 = 0x4000_0000;
const FIRST_DICTIONARY_NAME_LENGTH: usize = 10;
const FIRST_DICTIONARY_NAME: usize = 12;
const ROW_REFERENCE_PAGE_SHIFT: u32 = 8;
const SHORT_BYTES: usize = size_of::<u16>();
const LONG_BYTES: usize = size_of::<u32>();

/// Exact prior Number AFTER, retaining the original source-class dictionary.
pub fn source(case: Source) -> Vec<u8> {
    let initial = initial(case);
    let column = if matches!(case, Source::IdOnly) {
        "Score"
    } else {
        "Extra"
    };
    first(&initial, case, column)
}

/// Add one complete MR2 body row, retaining all historical LVAL rows literally.
pub fn append(before: &[u8], name: &str) -> Vec<u8> {
    assert!(before.len() == 11 * PAGE || before.len() == 12 * PAGE);
    let property_page = before.len() / PAGE - 1;
    let definition =
        canonical_created_add_column::append(&before[..ORIGINAL_PAGE_COUNT * PAGE], name);
    let mut after = before.to_vec();
    let definition_span = DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE;
    after[definition_span.clone()].copy_from_slice(&definition[definition_span]);

    let property = &before[property_page * PAGE..(property_page + 1) * PAGE];
    let count = word(property, ROW_COUNT_OFFSET);
    let old_start = word(property, DIRECTORY + SHORT_BYTES * (count - 1));
    let old_end = if count == 1 {
        PAGE
    } else {
        word(property, DIRECTORY + SHORT_BYTES * (count - 2))
    };
    let mut body = property[old_start..old_end].to_vec();
    assert_eq!(&body[..4], b"MR2\0");
    let first_length = word(&body, FIRST_DICTIONARY_NAME_LENGTH);
    let first_name = String::from_utf16(
        &body[FIRST_DICTIONARY_NAME..FIRST_DICTIONARY_NAME + first_length]
            .as_chunks::<SHORT_BYTES>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(first_name == "Required" || first_name == "DefaultValue");
    let dictionary = if first_name == "Required" {
        ["Required", "DefaultValue"]
    } else {
        ["DefaultValue", "Required"]
    };
    body.extend(number_group(name, true, &dictionary));
    let directory_end = DIRECTORY + SHORT_BYTES * (count + 1);
    let fitted = body.len() + directory_end <= old_start;
    let (target_page, target_slot, target_end) = if fitted {
        (property_page, count, old_start)
    } else {
        assert_eq!(
            property_page, 10,
            "only one immediate-EOF rollover is modeled"
        );
        assert_eq!(
            &before[2 * PAGE + 802..2 * PAGE + 812],
            &[14, 0, 16, 3, 0, 0, 17, 3, 0, 0,]
        );
        // These literal row cells are independent observations of MSysObjects'
        // owned/candidate maps, not production format offsets.
        assert_eq!(before[3 * PAGE + 3881], 4);
        assert_eq!(before[3 * PAGE + 3868], 4);
        after[3 * PAGE + 3881] = 12;
        after[3 * PAGE + 3868] = 8;
        let mut page = vec![0; PAGE];
        page[..2].copy_from_slice(&[1, 1]);
        page[4..8].copy_from_slice(b"LVAL");
        after.extend(page);
        (11, 0, PAGE)
    };
    let start = target_end.checked_sub(body.len()).unwrap();
    let directory_end = DIRECTORY + SHORT_BYTES * (target_slot + 1);
    assert!(start >= directory_end);
    let page = &mut after[target_page * PAGE..(target_page + 1) * PAGE];
    assert!(page[start..target_end].iter().all(|byte| *byte == 0));
    page[start..target_end].copy_from_slice(&body);
    put16(page, DIRECTORY + SHORT_BYTES * target_slot, start as u16);
    put16(page, ROW_COUNT_OFFSET, (target_slot + 1) as u16);
    put16(page, FREE_SPACE_OFFSET, (start - directory_end) as u16);

    let catalog = &mut after[CATALOG_PAGE * PAGE..(CATALOG_PAGE + 1) * PAGE];
    let root = word(catalog, DIRECTORY + SHORT_BYTES * LOGICAL_CATALOG_SLOT);
    let slot = if root & FORWARD_FLAG != 0 {
        FORWARD_TARGET_SLOT
    } else {
        LOGICAL_CATALOG_SLOT
    };
    let start = word(catalog, DIRECTORY + SHORT_BYTES * slot) & OFFSET_MASK;
    let end = word(catalog, DIRECTORY + SHORT_BYTES * (slot - 1)) & OFFSET_MASK;
    let row = &mut catalog[start..end];
    let footer = row.len() - MASK_BYTES - SHORT_BYTES;
    let descriptor = word(row, footer - SHORT_BYTES * (PROPERTY_VARIABLE + 1));
    put32(row, descriptor, SINGLE_PROPERTY_FLAG | body.len() as u32);
    put32(
        row,
        descriptor + LONG_BYTES,
        (target_page as u32) << ROW_REFERENCE_PAGE_SHIFT | target_slot as u32,
    );
    after
}
