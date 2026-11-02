//! Generic/raw and full read-only action Source qualification; Text mutation stays closed.
use crate::{
    IndexCheckStatus, NativeImage, PayloadPresence, TableExtraCoverage, check_table_constraints,
    inspect_table_indexes, inspect_table_schema, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(226);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn texts() -> [Option<&'static str>; 13] {
    [
        None,
        Some(""),
        Some("a"),
        Some("A"),
        Some("é"),
        Some("e\u{301}"),
        Some("Straße"),
        Some("Strasse"),
        Some("a  "),
        Some("😀"),
        Some("\0"),
        Some(""),
        Some("カタカナ"),
    ]
}

fn rows() -> Vec<Vec<Value>> {
    (0..24)
        .map(|ordinal| {
            let position = ordinal * 7 % 13;
            let value = if position == 11 {
                Value::Text("a".repeat(255))
            } else {
                texts()[position].map_or(Value::Null, |text| Value::Text(text.to_owned()))
            };
            vec![Value::Int32(ordinal as i32 + 1), value]
        })
        .collect()
}

fn updates(recipe: &str) -> Vec<(usize, Value)> {
    let text = |value: &str| Value::Text(value.to_owned());
    match recipe {
        "null-to-empty" => vec![(0, text(""))],
        "empty-to-null" => vec![(2, Value::Null)],
        "clear-present" => vec![(1, Value::Null)],
        "existing-family" => vec![(1, text("a"))],
        "case-carrier" => vec![(4, text("A"))],
        "accent-existing" => vec![(10, text("é"))],
        "trailing-existing" => vec![(3, text("a"))],
        "duplicate-batch" => vec![(1, text("a")), (12, text("a"))],
        "duplicate-reverse" => vec![(12, text("a")), (1, text("a"))],
        "present-noop" => vec![(4, text("a"))],
        "empty-target-group" => Vec::new(),
        _ => panic!("closed source recipe"),
    }
}

fn u16_at(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()))
}

fn frame(raw: &[u8], data: usize, slot: usize) -> (usize, usize) {
    let page = data * 4096;
    assert_eq!(&raw[page..page + 2], &[1, 1]);
    assert!(slot < u16_at(raw, page + 12));
    let start = u16_at(raw, page + 14 + 2 * slot);
    let end = if slot == 0 {
        4096
    } else {
        u16_at(raw, page + 12 + 2 * slot)
    };
    assert_eq!(start & !8191, 0);
    assert!(start < end && end <= 4096);
    (page + start, page + end)
}

fn entries(raw: &[u8], root: usize, data: usize) -> Vec<(Vec<u8>, usize)> {
    let page = &raw[root * 4096..(root + 1) * 4096];
    assert_eq!(&page[..2], &[4, 1]);
    assert_eq!(u16_at(page, 24), 0);
    let mut result = Vec::new();
    let mut start = 0;
    for end in 1..=3616 {
        if page[27 + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let entry = &page[480 + start..480 + end];
        assert!(entry.len() >= 5);
        let reference = u32::from_be_bytes(entry[entry.len() - 4..].try_into().unwrap());
        assert_eq!(reference >> 8, data as u32);
        result.push((
            entry[..entry.len() - 4].to_vec(),
            (reference & 255) as usize,
        ));
        start = end;
    }
    assert_eq!(result.len(), 24);
    assert_eq!(u16_at(page, 2), 3616 - start);
    let mut slots = result.iter().map(|entry| entry.1).collect::<Vec<_>>();
    slots.sort_unstable();
    assert_eq!(slots, (0..24).collect::<Vec<_>>());
    result
}

fn literal(before: &[u8], td: usize, recipe: &str) -> (Vec<u8>, Vec<Vec<Value>>) {
    let data = td + 5;
    let original_rows = rows();
    let changes = updates(recipe);
    let mut final_rows = original_rows.clone();
    let mut after = before.to_vec();
    for (slot, value) in &changes {
        let (start, end) = frame(before, data, *slot);
        let original = &before[start..end];
        assert_eq!(u16_at(original, 0), 2);
        assert_eq!(
            i32::from_le_bytes(original[2..6].try_into().unwrap()),
            *slot as i32 + 1
        );
        let payload = match value {
            Value::Null => Vec::new(),
            Value::Text(text) => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            _ => panic!("source Text/NULL carrier only"),
        };
        assert!(payload.len() <= original.len() - 13);
        after[start + 6..start + 6 + payload.len()].copy_from_slice(&payload);
        after[end - 7..end - 5]
            .copy_from_slice(&u16::try_from(6 + payload.len()).unwrap().to_le_bytes());
        after[end - 1] = if matches!(value, Value::Null) { 1 } else { 3 };
        final_rows[*slot][1] = value.clone();
    }
    let changed = final_rows != original_rows;
    for root in [td + 3, td + 4] {
        let original = entries(before, root, data);
        let mut final_entries = original
            .iter()
            .map(|(_, slot)| {
                let original_slot = original_rows
                    .iter()
                    .position(|row| row[1] == final_rows[*slot][1])
                    .unwrap();
                let component = original
                    .iter()
                    .find(|entry| entry.1 == original_slot)
                    .unwrap()
                    .0
                    .clone();
                let mut wire = component;
                wire.extend_from_slice(&(((data as u32) << 8) | *slot as u32).to_be_bytes());
                wire
            })
            .collect::<Vec<_>>();
        final_entries.sort_unstable();
        if !changed {
            continue;
        }
        let base = root * 4096;
        let mut bitmap = [0; 453];
        let mut active = Vec::new();
        for wire in final_entries {
            active.extend_from_slice(&wire);
            let end = active.len();
            assert!(end <= 3616);
            bitmap[end / 8] |= 1 << (end % 8);
        }
        after[base + 2..base + 4]
            .copy_from_slice(&u16::try_from(3616 - active.len()).unwrap().to_le_bytes());
        after[base + 27..base + 480].copy_from_slice(&bitmap);
        after[base + 480..base + 480 + active.len()].copy_from_slice(&active);
    }
    (after, final_rows)
}

fn inspect(raw: &[u8], table: &str, td: u32, version: u8, expected: &[Vec<Value>]) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let catalog = inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "MSysObjects",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(catalog.columns().len(), 17);
    let snapshot = read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
    assert_eq!(snapshot.rows(), expected);
    let schema = snapshot.schema();
    assert_eq!(schema.definition_page(), td);
    assert_eq!(schema.columns().len(), 2);
    let field = &schema.columns()[1];
    assert_eq!(
        (
            field.name(),
            field.native_type(),
            field.storage_length(),
            field.flags()
        ),
        ("KeyValue", 10, 510, 2)
    );
    let order = field.text_sort_order().unwrap();
    assert_eq!(
        (order.lcid(), order.variant(), order.version()),
        (1033, 0, version)
    );
    let descriptor = td as usize * 4096 + 124;
    assert_eq!(&raw[descriptor + 15..descriptor + 17], &[2, 0]);
    let constraints =
        check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| false)
            .unwrap();
    assert_eq!(constraints.rows(), 24);
    assert!(constraints.relationships().is_empty());
    let checked = crate::table::indexes::check_table_retained(
        &mut image.reader(),
        SCOPE,
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 24);
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let indexes =
        inspect_table_indexes(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
    for (name, physical, root, descending) in [
        ("Primary", 0, td + 2, false),
        ("Asc", 1, td + 3, false),
        ("Twin", 1, td + 3, false),
        ("Desc", 2, td + 4, true),
    ] {
        let index = indexes
            .indexes()
            .iter()
            .find(|index| index.name() == name)
            .unwrap();
        assert_eq!(
            (
                index.data_number(),
                index.root_page(),
                index.columns()[0].descending()
            ),
            (physical, root, descending)
        );
        let read = read_table_index(
            &mut image.reader(),
            SCOPE,
            table,
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let order = entries(raw, root as usize, td as usize + 5)
            .into_iter()
            .map(|(_, slot)| expected[slot].clone())
            .collect::<Vec<_>>();
        assert_eq!(read.rows(), order);
    }
    let coverage = image
        .inspect_table_macro_coverage(schema.object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
    assert_eq!(
        coverage.table_extra().coverage(),
        TableExtraCoverage::NotPresent
    );
    assert!(coverage.children().is_empty());
    let certificate = super::super::no_required_programs::NoRequiredTablePrograms::capture_actions(
        &image,
        schema,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    certificate
        .require_source(&image, schema, &mut budget(), &|| false)
        .unwrap();
    let source =
        super::super::normal_actions::capture(&image, table, &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), expected);
    assert_eq!(source.table.schema().object_id(), schema.object_id());
    assert_eq!(source.table.schema().definition_page(), td);
    assert_eq!(source.records.len(), 24);
    assert_eq!(source.indexes.indexes().len(), 4);
    for (ordinal, record) in source.records.iter().enumerate() {
        let (start, end) = frame(raw, td as usize + 5, ordinal);
        assert_eq!(record.reference, ((td + 5) << 8) | ordinal as u32);
        assert_eq!(record.offset, start);
        assert_eq!(record.data, raw[start..end]);
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires 34 pinned Text views; full read-only Source, no Text writer"]
fn text_views_have_exact_raw_generic_reads_and_complete_action_source() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        hash(&fs::read(directory.join("manifest.json")).unwrap()),
        "710051d16eefd9c7a08709b67f8291c7938f09b4bd2a971115c2b970f577868a"
    );
    let mut views = Vec::new();
    for (id, filename, length, digest) in [
        (
            "h5-original",
            "Encoded-V2016-before.accdb",
            1163264,
            "e2b5193dc4eb6c024e1e2883e894d64b4ff82c435ca441f0745e677615fc9276",
        ),
        (
            "h5-textlegacyleaf-null-to-empty",
            "h5-textlegacyleaf-null-to-empty.accdb",
            1163264,
            "8c57f21d0016ebc715ee94241bdfa5371cce74c4b7f24cfd5cad4a3eac89e5e9",
        ),
        (
            "h5-textlegacyleaf-empty-to-null",
            "h5-textlegacyleaf-empty-to-null.accdb",
            1163264,
            "f2201654d15fef7ceae58ac5db2787d023e317220c0d3de5c7af7774a94baf72",
        ),
        (
            "h5-textlegacyleaf-clear-present",
            "h5-textlegacyleaf-clear-present.accdb",
            1163264,
            "4881096c09edee6731dc56bde36adb5206bf5c83d95a42ce40615b9042338600",
        ),
        (
            "h5-textlegacyleaf-existing-family",
            "h5-textlegacyleaf-existing-family.accdb",
            1163264,
            "a257492a06cf4e1d980c7d7384c7a4e8cda08949ea262722f9f1ea14ff9e6467",
        ),
        (
            "h5-textlegacyleaf-case-carrier",
            "h5-textlegacyleaf-case-carrier.accdb",
            1163264,
            "8993ada98371230d59552fc21e8dcd26d654e8ee182e4c18254864917154cfe4",
        ),
        (
            "h5-textlegacyleaf-accent-existing",
            "h5-textlegacyleaf-accent-existing.accdb",
            1163264,
            "a2e065eb5d5ab09a8c36a4ef2a5d94ccbc0c36a52ca28be02042338ed5f74586",
        ),
        (
            "h5-textlegacyleaf-trailing-existing",
            "h5-textlegacyleaf-trailing-existing.accdb",
            1163264,
            "a2b6470d87394ab5b1d681ee9820f7b7075d6775ef072cded5389968e39708ec",
        ),
        (
            "h5-textlegacyleaf-duplicate-batch",
            "h5-textlegacyleaf-duplicate-batch.accdb",
            1163264,
            "0eb31a63a449bebd807b3342e7ab2115db3de22ec64395753ed383534041955b",
        ),
        (
            "h5-textmodernleaf-null-to-empty",
            "h5-textmodernleaf-null-to-empty.accdb",
            1163264,
            "d72389e0d6b88d82056c616c0ff689edeeedad6decdfb60e9e09034ecf85c752",
        ),
        (
            "h5-textmodernleaf-empty-to-null",
            "h5-textmodernleaf-empty-to-null.accdb",
            1163264,
            "4cf87b061a7a4b1e81a8cdea2e6df98a8a8220a198b6f719848ca45cd53ec0ee",
        ),
        (
            "h5-textmodernleaf-clear-present",
            "h5-textmodernleaf-clear-present.accdb",
            1163264,
            "3f9067ede04e8a48cd8d208de2eab06c15cfcbe691a79ac0a418effc8816daa6",
        ),
        (
            "h5-textmodernleaf-existing-family",
            "h5-textmodernleaf-existing-family.accdb",
            1163264,
            "e0319e523b55713c7fc810a5e2520b497568aaeb861f6c95787bdcafd5c3355f",
        ),
        (
            "h5-textmodernleaf-case-carrier",
            "h5-textmodernleaf-case-carrier.accdb",
            1163264,
            "e434d49c4a839e35414b3fad653f7f1d36a29f16bbf829d22c86e5501dd7c71b",
        ),
        (
            "h5-textmodernleaf-accent-existing",
            "h5-textmodernleaf-accent-existing.accdb",
            1163264,
            "95cc5e26d1a876ebd3023c8ccf37ca583e0be6e3be379d6306e39d3b61ae6455",
        ),
        (
            "h5-textmodernleaf-trailing-existing",
            "h5-textmodernleaf-trailing-existing.accdb",
            1163264,
            "8f7c207d3c35c7589e4323f0faf926adf7dd8485820d8702a22bdd633d5648a5",
        ),
        (
            "h5-textmodernleaf-duplicate-batch",
            "h5-textmodernleaf-duplicate-batch.accdb",
            1163264,
            "998f717bc58a297ffb8666e63f9d879094e1c4a2704055dc1b17995136f7ccde",
        ),
        (
            "h6-original",
            "Encoded-V2019-before.accdb",
            1236992,
            "636e137ee3a1d51f6525072de87963cb33a03b01a253be1d2124cf6bb4746279",
        ),
        (
            "h6-textlegacyleaf-null-to-empty",
            "h6-textlegacyleaf-null-to-empty.accdb",
            1236992,
            "8d283e5abc0d4bf4f2e44347b3eb59ba96ab0173684bc530d517ea1a3c630bc7",
        ),
        (
            "h6-textlegacyleaf-empty-to-null",
            "h6-textlegacyleaf-empty-to-null.accdb",
            1236992,
            "083dbcbd1e2e719dc81221fd3c291242df6c0bf1ea8c2d0ae11db3b77cd86a91",
        ),
        (
            "h6-textlegacyleaf-clear-present",
            "h6-textlegacyleaf-clear-present.accdb",
            1236992,
            "3c8d35d03366d3ce2375e00a65d0ad7a6dacfc7a97e19553ff2844baa383a402",
        ),
        (
            "h6-textlegacyleaf-existing-family",
            "h6-textlegacyleaf-existing-family.accdb",
            1236992,
            "d11365a77dacaf7a6ace24e76d54d69fd0167df814e222dedd132f90b0149abd",
        ),
        (
            "h6-textlegacyleaf-case-carrier",
            "h6-textlegacyleaf-case-carrier.accdb",
            1236992,
            "a370cdc949811575b319363ab33662f502eae2a7761af2cc284ac43d7d66ecdf",
        ),
        (
            "h6-textlegacyleaf-accent-existing",
            "h6-textlegacyleaf-accent-existing.accdb",
            1236992,
            "6c5adb5a6af4c32066c2353ef26a9663e230679aa8b501d4e074ae0a84a4adb9",
        ),
        (
            "h6-textlegacyleaf-trailing-existing",
            "h6-textlegacyleaf-trailing-existing.accdb",
            1236992,
            "aa465885410ea7d3a552a7b14eb6fc98fdd0b58d773acdbcd37b5ab9814392f4",
        ),
        (
            "h6-textlegacyleaf-duplicate-batch",
            "h6-textlegacyleaf-duplicate-batch.accdb",
            1236992,
            "6483c4501d107b5b00dd4d4ffa314d239dc09c2b8afe9880f2538e263523cb3a",
        ),
        (
            "h6-textmodernleaf-null-to-empty",
            "h6-textmodernleaf-null-to-empty.accdb",
            1236992,
            "00eb46619945920cbfd247e8d18023af7df356ee07f6b582c79e5afa64efc5ae",
        ),
        (
            "h6-textmodernleaf-empty-to-null",
            "h6-textmodernleaf-empty-to-null.accdb",
            1236992,
            "e1412d959d65da97811e515609a20342c4a402776df30ce54e15788d2c1b49da",
        ),
        (
            "h6-textmodernleaf-clear-present",
            "h6-textmodernleaf-clear-present.accdb",
            1236992,
            "c9fa44ad18aa0af1fead55c3ccc63ca6bb9a096a1a7236a37426cbcb5b44ef0a",
        ),
        (
            "h6-textmodernleaf-existing-family",
            "h6-textmodernleaf-existing-family.accdb",
            1236992,
            "4412bebdae0164d1ed6a270f4928d1cf6b3ce3a8c8c77a7c31959c6bb62113a4",
        ),
        (
            "h6-textmodernleaf-case-carrier",
            "h6-textmodernleaf-case-carrier.accdb",
            1236992,
            "33b4d2465cbb1a9c4cbaefe004dd4d9bda3a785aff4e9c6cc62d603bc80273a9",
        ),
        (
            "h6-textmodernleaf-accent-existing",
            "h6-textmodernleaf-accent-existing.accdb",
            1236992,
            "642d9ad643fe1997a04f2a9ba34361849eb185ff6d73a170cde7cf7ae0ac7ccf",
        ),
        (
            "h6-textmodernleaf-trailing-existing",
            "h6-textmodernleaf-trailing-existing.accdb",
            1236992,
            "0d56811d75c693e4a828070b8b4f5d087f840ba9f234f15df3796c46c0c8c406",
        ),
        (
            "h6-textmodernleaf-duplicate-batch",
            "h6-textmodernleaf-duplicate-batch.accdb",
            1236992,
            "d21bd11353950fa3e8cc3d413086041e905bd7f9f0edf125f373f523395093ad",
        ),
    ] {
        let bytes = fs::read(directory.join(filename)).unwrap();
        assert_eq!(bytes.len(), length);
        assert_eq!(hash(&bytes), digest);
        views.push((id, bytes));
    }
    let cases = [
        (
            5_u8,
            "TextLegacyLeaf",
            "null-to-empty",
            "h5-textlegacyleaf-null-to-empty",
        ),
        (
            5_u8,
            "TextLegacyLeaf",
            "empty-to-null",
            "h5-textlegacyleaf-empty-to-null",
        ),
        (
            5_u8,
            "TextLegacyLeaf",
            "clear-present",
            "h5-textlegacyleaf-clear-present",
        ),
        (
            5_u8,
            "TextLegacyLeaf",
            "existing-family",
            "h5-textlegacyleaf-existing-family",
        ),
        (
            5_u8,
            "TextLegacyLeaf",
            "case-carrier",
            "h5-textlegacyleaf-case-carrier",
        ),
        (
            5_u8,
            "TextLegacyLeaf",
            "accent-existing",
            "h5-textlegacyleaf-accent-existing",
        ),
        (
            5_u8,
            "TextLegacyLeaf",
            "trailing-existing",
            "h5-textlegacyleaf-trailing-existing",
        ),
        (
            5_u8,
            "TextLegacyLeaf",
            "duplicate-batch",
            "h5-textlegacyleaf-duplicate-batch",
        ),
        (
            5_u8,
            "TextLegacyLeaf",
            "duplicate-reverse",
            "h5-textlegacyleaf-duplicate-batch",
        ),
        (5_u8, "TextLegacyLeaf", "present-noop", "h5-original"),
        (5_u8, "TextLegacyLeaf", "empty-target-group", "h5-original"),
        (
            5_u8,
            "TextModernLeaf",
            "null-to-empty",
            "h5-textmodernleaf-null-to-empty",
        ),
        (
            5_u8,
            "TextModernLeaf",
            "empty-to-null",
            "h5-textmodernleaf-empty-to-null",
        ),
        (
            5_u8,
            "TextModernLeaf",
            "clear-present",
            "h5-textmodernleaf-clear-present",
        ),
        (
            5_u8,
            "TextModernLeaf",
            "existing-family",
            "h5-textmodernleaf-existing-family",
        ),
        (
            5_u8,
            "TextModernLeaf",
            "case-carrier",
            "h5-textmodernleaf-case-carrier",
        ),
        (
            5_u8,
            "TextModernLeaf",
            "accent-existing",
            "h5-textmodernleaf-accent-existing",
        ),
        (
            5_u8,
            "TextModernLeaf",
            "trailing-existing",
            "h5-textmodernleaf-trailing-existing",
        ),
        (
            5_u8,
            "TextModernLeaf",
            "duplicate-batch",
            "h5-textmodernleaf-duplicate-batch",
        ),
        (
            5_u8,
            "TextModernLeaf",
            "duplicate-reverse",
            "h5-textmodernleaf-duplicate-batch",
        ),
        (5_u8, "TextModernLeaf", "present-noop", "h5-original"),
        (5_u8, "TextModernLeaf", "empty-target-group", "h5-original"),
        (
            6_u8,
            "TextLegacyLeaf",
            "null-to-empty",
            "h6-textlegacyleaf-null-to-empty",
        ),
        (
            6_u8,
            "TextLegacyLeaf",
            "empty-to-null",
            "h6-textlegacyleaf-empty-to-null",
        ),
        (
            6_u8,
            "TextLegacyLeaf",
            "clear-present",
            "h6-textlegacyleaf-clear-present",
        ),
        (
            6_u8,
            "TextLegacyLeaf",
            "existing-family",
            "h6-textlegacyleaf-existing-family",
        ),
        (
            6_u8,
            "TextLegacyLeaf",
            "case-carrier",
            "h6-textlegacyleaf-case-carrier",
        ),
        (
            6_u8,
            "TextLegacyLeaf",
            "accent-existing",
            "h6-textlegacyleaf-accent-existing",
        ),
        (
            6_u8,
            "TextLegacyLeaf",
            "trailing-existing",
            "h6-textlegacyleaf-trailing-existing",
        ),
        (
            6_u8,
            "TextLegacyLeaf",
            "duplicate-batch",
            "h6-textlegacyleaf-duplicate-batch",
        ),
        (
            6_u8,
            "TextLegacyLeaf",
            "duplicate-reverse",
            "h6-textlegacyleaf-duplicate-batch",
        ),
        (6_u8, "TextLegacyLeaf", "present-noop", "h6-original"),
        (6_u8, "TextLegacyLeaf", "empty-target-group", "h6-original"),
        (
            6_u8,
            "TextModernLeaf",
            "null-to-empty",
            "h6-textmodernleaf-null-to-empty",
        ),
        (
            6_u8,
            "TextModernLeaf",
            "empty-to-null",
            "h6-textmodernleaf-empty-to-null",
        ),
        (
            6_u8,
            "TextModernLeaf",
            "clear-present",
            "h6-textmodernleaf-clear-present",
        ),
        (
            6_u8,
            "TextModernLeaf",
            "existing-family",
            "h6-textmodernleaf-existing-family",
        ),
        (
            6_u8,
            "TextModernLeaf",
            "case-carrier",
            "h6-textmodernleaf-case-carrier",
        ),
        (
            6_u8,
            "TextModernLeaf",
            "accent-existing",
            "h6-textmodernleaf-accent-existing",
        ),
        (
            6_u8,
            "TextModernLeaf",
            "trailing-existing",
            "h6-textmodernleaf-trailing-existing",
        ),
        (
            6_u8,
            "TextModernLeaf",
            "duplicate-batch",
            "h6-textmodernleaf-duplicate-batch",
        ),
        (
            6_u8,
            "TextModernLeaf",
            "duplicate-reverse",
            "h6-textmodernleaf-duplicate-batch",
        ),
        (6_u8, "TextModernLeaf", "present-noop", "h6-original"),
        (6_u8, "TextModernLeaf", "empty-target-group", "h6-original"),
    ];
    let mut reconstructed = 0;
    let mut inspected = 0;
    for header in [5, 6] {
        let original = if header == 5 {
            "h5-original"
        } else {
            "h6-original"
        };
        let before = &views.iter().find(|view| view.0 == original).unwrap().1;
        for (table, version, td) in [
            ("TextLegacyLeaf", 0, if header == 5 { 89 } else { 106 }),
            ("TextModernLeaf", 1, if header == 5 { 96 } else { 113 }),
        ] {
            inspect(before, table, td, version, &rows());
            inspected += 1;
            for (_, _, recipe, view) in cases
                .iter()
                .filter(|case| case.0 == header && case.1 == table)
            {
                let after = &views
                    .iter()
                    .find(|candidate| candidate.0 == *view)
                    .unwrap()
                    .1;
                let (expected, final_rows) = literal(before, td as usize, recipe);
                assert_eq!(&expected, after);
                reconstructed += 1;
                if !matches!(
                    *recipe,
                    "duplicate-reverse" | "present-noop" | "empty-target-group"
                ) {
                    inspect(after, table, td, version, &final_rows);
                    inspected += 1;
                }
            }
        }
    }
    assert_eq!((views.len(), reconstructed, inspected), (34, 44, 36));
}

#[path = "text_update/actions.rs"]
mod actions;
