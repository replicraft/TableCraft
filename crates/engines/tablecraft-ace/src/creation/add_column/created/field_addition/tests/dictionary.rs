//! Independently permuted known keys remain valid inert data, outside source policy.
use super::super::FieldAdditionError;
use super::*;

const PAGE_BYTES: usize = 4096;
const PROPERTY_PAGE: usize = 10;
const FIRST_SLOT: usize = 14;
const OFFSET_MASK: usize = 0x1fff;
const SIGNATURE_BYTES: usize = 4;
const DICTIONARY_HEADER_BYTES: usize = 6;
const NAME_BYTES: usize = 2;
const BOOLEAN_ENTRY_BYTES: usize = 9;
const VALUE_DICTIONARY_INDEX: usize = 4;

#[test]
fn unevidenced_known_key_permutation_cannot_become_its_own_source_oracle() {
    let before = raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true);
    let mut changed = before.clone();
    let page = &mut changed[PROPERTY_PAGE * PAGE_BYTES..(PROPERTY_PAGE + 1) * PAGE_BYTES];
    let start = usize::from(u16::from_le_bytes(
        page[FIRST_SLOT..FIRST_SLOT + 2].try_into().unwrap(),
    )) & OFFSET_MASK;
    let body = &mut page[start..];
    assert_eq!(&body[..SIGNATURE_BYTES], b"MR2\0");
    let dictionary_length = u32::from_le_bytes(
        body[SIGNATURE_BYTES..SIGNATURE_BYTES + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let names_start = SIGNATURE_BYTES + DICTIONARY_HEADER_BYTES;
    let first_length = usize::from(u16::from_le_bytes(
        body[names_start..names_start + NAME_BYTES]
            .try_into()
            .unwrap(),
    ));
    let split = names_start + NAME_BYTES + first_length;
    let end = SIGNATURE_BYTES + dictionary_length;
    let required = body[names_start..split].to_vec();
    let allow_empty = body[split..end].to_vec();
    assert_eq!(
        required[NAME_BYTES..],
        "Required"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        allow_empty[NAME_BYTES..],
        "AllowZeroLength"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    body[names_start..names_start + allow_empty.len()].copy_from_slice(&allow_empty);
    body[names_start + allow_empty.len()..end].copy_from_slice(&required);

    // The only field group has two Boolean entries. Swap both complete entries
    // and repair their explicit dictionary indices to author AZL,Required.
    let entries = body.len() - 2 * BOOLEAN_ENTRY_BYTES;
    let mut required_value = body[entries..entries + BOOLEAN_ENTRY_BYTES].to_vec();
    let mut empty_value = body[entries + BOOLEAN_ENTRY_BYTES..].to_vec();
    required_value[VALUE_DICTIONARY_INDEX..VALUE_DICTIONARY_INDEX + 2]
        .copy_from_slice(&1_u16.to_le_bytes());
    empty_value[VALUE_DICTIONARY_INDEX..VALUE_DICTIONARY_INDEX + 2]
        .copy_from_slice(&0_u16.to_le_bytes());
    body[entries..entries + BOOLEAN_ENTRY_BYTES].copy_from_slice(&empty_value);
    body[entries + BOOLEAN_ENTRY_BYTES..].copy_from_slice(&required_value);

    let source = image(&changed);
    let properties = crate::inspect_table_properties(
        &mut source.reader(),
        CatalogScope::new(901),
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    crate::properties::framing::validate(&properties, &mut budget(), &|| false).unwrap();
    assert_eq!(
        properties.groups()[0].properties()[0].name(),
        "AllowZeroLength"
    );
    assert_eq!(properties.groups()[0].properties()[1].name(), "Required");
    assert!(matches!(
        source.check_created_field_addition_source("Table1", &mut budget(), &|| false),
        Err(FieldAdditionError::Source)
    ));
    assert_eq!(
        before,
        raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true)
    );
}
