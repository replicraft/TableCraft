//! Literal Text cardinality source checks; no mutation planner or serializer.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_indexes,
    inspect_table_schema, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(233);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn rows() -> Vec<Vec<Value>> {
    let values = [
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
    ];
    (0..24)
        .map(|ordinal| {
            let position = ordinal * 7 % 13;
            let value = if position == 11 {
                Value::Text("a".repeat(255))
            } else {
                values[position].map_or(Value::Null, |text| Value::Text(text.to_owned()))
            };
            vec![Value::Int32(ordinal as i32 + 1), value]
        })
        .collect()
}

fn u16_at(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()))
}

fn frame(raw: &[u8], data: usize, slot: usize) -> (usize, usize) {
    let base = data * 4096;
    assert_eq!(&raw[base..base + 2], &[1, 1]);
    assert!(slot < u16_at(raw, base + 12));
    let start = u16_at(raw, base + 14 + slot * 2) & 8191;
    let end = if slot == 0 {
        4096
    } else {
        u16_at(raw, base + 12 + slot * 2) & 8191
    };
    assert!(start < end && end <= 4096);
    (base + start, base + end)
}

fn entries(raw: &[u8], root: usize, data: usize) -> Vec<(Vec<u8>, usize)> {
    let page = &raw[root * 4096..(root + 1) * 4096];
    assert_eq!(&page[..2], &[4, 1]);
    assert_eq!(u16_at(page, 24), 0);
    let mut output = Vec::new();
    let mut start = 0;
    for end in 1..=3616 {
        if page[27 + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let wire = &page[480 + start..480 + end];
        assert!(wire.len() >= 5);
        let reference = u32::from_be_bytes(wire[wire.len() - 4..].try_into().unwrap());
        assert_eq!(reference >> 8, data as u32);
        output.push((wire[..wire.len() - 4].to_vec(), (reference & 255) as usize));
        start = end;
    }
    assert_eq!(u16_at(page, 2), 3616 - start);
    assert!(
        output
            .windows(2)
            .all(|pair| { (&pair[0].0, pair[0].1) < (&pair[1].0, pair[1].1) })
    );
    output
}

fn encoded_record(row: &[Value]) -> Vec<u8> {
    let Value::Int32(id) = row[0] else {
        panic!("manual I32 ID")
    };
    let mut raw = vec![2, 0];
    raw.extend_from_slice(&id.to_le_bytes());
    match &row[1] {
        Value::Null => {}
        Value::Text(text) => raw.extend(text.encode_utf16().flat_map(u16::to_le_bytes)),
        _ => panic!("literal Text/NULL"),
    }
    let eod = u16::try_from(raw.len()).unwrap();
    raw.extend_from_slice(&eod.to_le_bytes());
    raw.extend_from_slice(&[6, 0, 1, 0]);
    raw.push(if matches!(row[1], Value::Null) { 1 } else { 3 });
    raw
}

fn literal(before: &[u8], td: usize, version: u8) -> (Vec<u8>, Vec<(usize, Vec<Value>)>) {
    let original = rows();
    let mut states = original.iter().cloned().enumerate().collect::<Vec<_>>();
    let data = td + 5;
    for (slot, row) in original.iter().enumerate() {
        let (start, end) = frame(before, data, slot);
        assert_eq!(before[start..end], encoded_record(row));
    }
    let mut after = before.to_vec();
    let (start, end) = frame(before, data, 1);
    assert_eq!(end - start, 27);
    assert_eq!(original[1][1], Value::Text("Strasse".into()));
    after[start + 6..start + 10].copy_from_slice(&[0x73, 0, 0x73, 0]);
    after[end - 7..end - 5].copy_from_slice(&10_u16.to_le_bytes());
    states[1].1[1] = Value::Text("ss".into());
    for (root, descending) in [(td + 3, false), (td + 4, true)] {
        let component = match (version, descending) {
            (0, false) => vec![0x7f, 0x6b, 0x6b, 1, 0],
            (0, true) => vec![0x80, 0x94, 0x94, 0xfe, 0xff, 0],
            (1, false) => vec![0x7f, 0x0e, 0x91, 0x0e, 0x91, 1, 0],
            (1, true) => vec![0x80, 0xf1, 0x6e, 0xf1, 0x6e, 0xfe, 0xff, 0],
            _ => panic!("qualified General versions only"),
        };
        let prior = entries(before, root, data);
        assert_eq!(prior.len(), 24);
        assert!(prior.iter().all(|(key, _)| key != &component));
        let mut wires = prior
            .into_iter()
            .map(|(prior, slot)| {
                let mut value = if slot == 1 { component.clone() } else { prior };
                value.extend_from_slice(&(((data as u32) << 8) | slot as u32).to_be_bytes());
                value
            })
            .collect::<Vec<_>>();
        wires.sort_unstable();
        let mut active = Vec::new();
        let mut mask = [0; 453];
        for wire in wires {
            active.extend_from_slice(&wire);
            let end = active.len();
            assert!(end <= 3616);
            mask[end / 8] |= 1 << (end % 8);
        }
        let base = root * 4096;
        after[base + 2..base + 4]
            .copy_from_slice(&u16::try_from(3616 - active.len()).unwrap().to_le_bytes());
        after[base + 27..base + 480].copy_from_slice(&mask);
        after[base + 480..base + 480 + active.len()].copy_from_slice(&active);
    }
    // No historical count is written. This is an existing-entry UPDATE, not INSERT.
    (after, states)
}

fn inspect(raw: &[u8], table: &str, td: u32, version: u8, states: &[(usize, Vec<Value>)]) {
    let expected = states
        .iter()
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
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
    let sort = field.text_sort_order().unwrap();
    assert_eq!(
        (sort.lcid(), sort.variant(), sort.version()),
        (1033, 0, version)
    );
    let constraints =
        check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| false)
            .unwrap();
    assert_eq!(constraints.rows(), states.len());
    assert!(constraints.relationships().is_empty());
    let checked = crate::table::indexes::check_table_retained(
        &mut image.reader(),
        SCOPE,
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), states.len());
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
            .map(|(_, slot)| {
                states
                    .iter()
                    .find(|(original_slot, _)| *original_slot == slot)
                    .unwrap()
                    .1
                    .clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(read.rows(), order);
    }
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
    assert_eq!(source.records.len(), states.len());
    for (record, (slot, _)) in source.records.iter().zip(states) {
        let (start, end) = frame(raw, td as usize + 5, *slot);
        assert_eq!(record.reference, ((td + 5) << 8) | *slot as u32);
        assert_eq!(record.offset, start);
        assert_eq!(record.data, raw[start..end]);
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires6 independently modeled new-family Text UPDATE views; source-only"]
fn new_text_family_update_keeps_history_and_has_complete_readonly_source() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_NEW_KEY_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        hash(&fs::read(directory.join("manifest.json")).unwrap()),
        "3b22d4be6b6730dce7f19da23e103c794818e86380ceb24f8a9d6da09a6cea43"
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
            "h5-textlegacyleaf-new-family-ss",
            "h5-textlegacyleaf-new-family-ss.accdb",
            1163264,
            "504fdd0ca3f0ec53f933425c3e91565af431def3bdc22a0aeee47caa22081c36",
        ),
        (
            "h5-textmodernleaf-new-family-ss",
            "h5-textmodernleaf-new-family-ss.accdb",
            1163264,
            "ad75b859be057c8bb30efb55378a9e1c7bcf58e5826e6c67b98feab3e8d17912",
        ),
        (
            "h6-original",
            "Encoded-V2019-before.accdb",
            1236992,
            "636e137ee3a1d51f6525072de87963cb33a03b01a253be1d2124cf6bb4746279",
        ),
        (
            "h6-textlegacyleaf-new-family-ss",
            "h6-textlegacyleaf-new-family-ss.accdb",
            1236992,
            "4e25b090c33a4a66edf70261c2afc95204e326fb3de4a69291258033ceaaa7dc",
        ),
        (
            "h6-textmodernleaf-new-family-ss",
            "h6-textmodernleaf-new-family-ss.accdb",
            1236992,
            "f89c230bcac0d0113949fc7cc8e2a9d440b4504ee85378f99633b93a0aacbc20",
        ),
    ] {
        let raw = fs::read(directory.join(filename)).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(hash(&raw), digest);
        views.push((id, raw));
    }
    let mut contexts = 0;
    for (header, table, td, version, identity) in [
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "h5-textlegacyleaf-new-family-ss",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "h5-textmodernleaf-new-family-ss",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "h6-textlegacyleaf-new-family-ss",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "h6-textmodernleaf-new-family-ss",
        ),
    ] {
        let original = format!("h{header}-original");
        let before = &views.iter().find(|(id, _)| *id == original).unwrap().1;
        let states = rows().into_iter().enumerate().collect::<Vec<_>>();
        inspect(before, table, td, version, &states);
        let (expected, states) = literal(before, td as usize, version);
        let after = &views.iter().find(|(id, _)| *id == identity).unwrap().1;
        assert_eq!(&expected, after);
        inspect(after, table, td, version, &states);
        contexts += 2;
    }
    assert_eq!(contexts, 8);
    assert_eq!(views.len(), 6);
}
