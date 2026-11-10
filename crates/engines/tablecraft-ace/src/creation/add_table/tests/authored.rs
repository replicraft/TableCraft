//! Actual source catalog gaps select native property placement before planning.
use crate::{
    AddTableSourceProfile, CreateColumn, FirstTableDefinition, NativeImage, PropertyGroupKind,
    add_table_source_capabilities, authored_add_table_source_capabilities,
    inspect_table_properties, read_table,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::retained_catalog_gap::{catalog_gap, source};

const SCOPE: CatalogScope = CatalogScope::new(7361);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 10_000_000, 512 * 1024 * 1024)
}

fn word(bytes: &[u8], offset: usize) -> usize {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}

fn map_record(bytes: &[u8], page: usize, slot: usize) -> &[u8] {
    let page = &bytes[page * 4096..(page + 1) * 4096];
    assert!(slot < word(page, 12));
    let start = word(page, 14 + slot * 2) & 0x1fff;
    let end = if slot == 0 {
        4096
    } else {
        word(page, 14 + (slot - 1) * 2) & 0x1fff
    };
    &page[start..end]
}

fn expected_bitmap(inline: &[u8], additional: usize) -> Vec<u8> {
    assert_eq!(inline[0], 0);
    let base = long(inline, 1);
    let mut bitmap = vec![0; 4096];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    for (byte_index, byte) in inline[5..].iter().enumerate() {
        for bit in 0..8 {
            if byte & (1 << bit) != 0 {
                let member = base + byte_index * 8 + bit;
                bitmap[4 + member / 8] |= 1 << (member % 8);
            }
        }
    }
    bitmap[4 + additional / 8] |= 1 << (additional % 8);
    bitmap
}

fn assert_promoted_property_maps(before: &[u8], after: &[u8]) {
    let root = before.len() / 4096;
    assert!(root > 64);
    let property = root + 3;
    assert_eq!(after.len(), before.len() + 6 * 4096);
    let mut bitmap_pages = Vec::new();
    for (slot, expected_page) in [(16, root + 4), (17, root + 5)] {
        let original = map_record(before, 3, slot);
        let promoted = map_record(after, 3, slot);
        assert_eq!(promoted.len(), original.len());
        assert_eq!(promoted[0], 1);
        let number = long(promoted, 1);
        assert_eq!(number, expected_page);
        assert!(promoted[5..].iter().all(|byte| *byte == 0));
        bitmap_pages.push(number);
        assert_eq!(
            &after[number * 4096..(number + 1) * 4096],
            expected_bitmap(original, property),
            "complete bitmap for property map slot {slot}",
        );
    }
    assert_ne!(bitmap_pages[0], bitmap_pages[1]);
    let free = map_record(after, 1, 0);
    assert_eq!(free[0], 0);
    let base = long(free, 1);
    for member in root..root + 6 {
        let bit = member - base;
        assert_eq!(
            free[5 + bit / 8] & (1 << (bit % 8)),
            0,
            "allocated appended page {member}"
        );
    }
}

fn name(text: &str) -> Vec<u8> {
    let wide = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut bytes = (wide.len() as u16).to_le_bytes().to_vec();
    bytes.extend(wide);
    bytes
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((6 + payload.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn properties(description: &str) -> Vec<u8> {
    let mut dictionary = name("Required");
    dictionary.extend(name("DefaultValue"));
    dictionary.extend(name("Description"));
    let label = name("Score");
    let mut group = ((4 + label.len()) as u32).to_le_bytes().to_vec();
    group.extend(label);
    group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]);
    group.extend([10, 0, 1, 12, 1, 0, 2, 0, b'0', 0]);
    let text = description
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    group.extend(((8 + text.len()) as u16).to_le_bytes());
    group.extend([1, 10, 2, 0]);
    group.extend((text.len() as u16).to_le_bytes());
    group.extend(text);
    let mut bytes = b"MR2\0".to_vec();
    bytes.extend(block(128, &dictionary));
    bytes.extend(block(1, &group));
    bytes
}

#[test]
fn authored_append_uses_actual_catalog_gap_for_small_declaration_properties() {
    let before = source();
    assert!(
        add_table_source_capabilities(&before, SCOPE, &mut budget(), &|| false)
            .unwrap()
            .can_add_keyed()
    );
    let advisory =
        authored_add_table_source_capabilities(&before, SCOPE, &mut budget(), &|| false).unwrap();
    assert!(advisory.can_add_keyed());
    assert!(!advisory.requires_native());

    let description = "D".repeat(255);
    let zero = Value::Int32(0);
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32)
            .with_required(false)
            .with_default_literal(&zero)
            .unwrap()
            .with_description(&description)
            .unwrap(),
    ];
    let declaration =
        FirstTableDefinition::capture_authored("Described", &columns, &mut budget(), &|| false)
            .unwrap();
    assert!(
        !declaration.is_native(),
        "the declaration fits the original canonical construction"
    );
    assert!(description.encode_utf16().count() * 2 > catalog_gap(&before));
    let image =
        NativeImage::capture_retained(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
            .unwrap();
    let plan = image
        .plan_authored_add_table(declaration, &mut budget(), &|| false)
        .unwrap();
    assert!(plan.is_native());
    assert_eq!(plan.source_profile(), AddTableSourceProfile::Retained);
    assert_eq!(
        &plan.encode_intent(&mut budget(), &|| false).unwrap()[..8],
        b"TCAT0007"
    );
    assert_eq!(plan.before_bytes(), before);
    assert_promoted_property_maps(&before, plan.after_bytes());
    for (number, page) in before.as_chunks::<4096>().0.iter().enumerate() {
        if [1, 2, 3, 4].contains(&number) {
            continue;
        }
        assert_eq!(
            &plan.after_bytes()[number * 4096..(number + 1) * 4096],
            page,
            "protected original page {number}"
        );
    }
    let observed = inspect_table_properties(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Described",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(observed.raw_bytes(), properties(&description));
    let group = observed.group(PropertyGroupKind::Column, "Score").unwrap();
    assert_eq!(
        group.get("Description").unwrap().text().unwrap(),
        description
    );
    assert!(!group.get("Required").unwrap().boolean().unwrap());
    assert_eq!(group.get("DefaultValue").unwrap().text().unwrap(), "0");
    let table = read_table(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Described",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.columns().len(), 2);
    assert!(table.rows().is_empty());
    let subsequent =
        authored_add_table_source_capabilities(plan.after_bytes(), SCOPE, &mut budget(), &|| false)
            .unwrap();
    assert!(subsequent.can_add_keyed());
    assert!(subsequent.can_add_keyless());
    assert!(subsequent.requires_native());
    let original =
        add_table_source_capabilities(plan.after_bytes(), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(!original.can_add_keyed());
    assert!(!original.can_add_keyless());
}
