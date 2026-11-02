//! Literal Text cardinality source checks; no mutation planner or serializer.
use crate::{
    IndexCheckStatus, NativeImage, PropertyGroupKind, check_table_constraints,
    inspect_table_indexes, inspect_table_properties, inspect_table_schema, read_table,
    read_table_index,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(232);

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
    vec![
        vec![Value::Int32(1), Value::Null],
        vec![Value::Int32(2), Value::Text("a".into())],
        vec![Value::Int32(3), Value::Text("A".into())],
    ]
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

fn literal(
    before: &[u8],
    td: usize,
    data: usize,
    recipe: &str,
) -> (Vec<u8>, Vec<(usize, Vec<Value>)>) {
    let original = rows();
    let mut states = original.iter().cloned().enumerate().collect::<Vec<_>>();
    for (slot, row) in original.iter().enumerate() {
        let (start, end) = frame(before, data, slot);
        assert_eq!(before[start..end], encoded_record(row));
    }
    let mut after = before.to_vec();
    if recipe == "empty-insert" {
        return (after, states);
    }
    assert!(matches!(recipe, "insert-empty" | "insert-null"));
    let value = if recipe == "insert-empty" {
        Value::Text(String::new())
    } else {
        Value::Null
    };
    let row = vec![Value::Int32(4), value];
    let raw = encoded_record(&row);
    assert_eq!(raw.len(), 13);
    after[data * 4096 + 4040..data * 4096 + 4053].copy_from_slice(&raw);
    after[data * 4096 + 20..data * 4096 + 22].copy_from_slice(&4040_u16.to_le_bytes());
    after[data * 4096 + 12..data * 4096 + 14].copy_from_slice(&4_u16.to_le_bytes());
    after[data * 4096 + 2..data * 4096 + 4].copy_from_slice(&4018_u16.to_le_bytes());
    after[td * 4096 + 16..td * 4096 + 20].copy_from_slice(&4_u32.to_le_bytes());
    states.push((3, row));
    for physical in 0..3 {
        let root = td + 2 + physical;
        let prior = entries(before, root, data);
        assert_eq!(prior.len(), 3);
        let mut wires = prior
            .into_iter()
            .map(|(mut component, slot)| {
                component.extend_from_slice(&(((data as u32) << 8) | slot as u32).to_be_bytes());
                component
            })
            .collect::<Vec<_>>();
        let mut new = if physical == 0 {
            vec![0x7f, 0x80, 0, 0, 4]
        } else if recipe == "insert-null" {
            vec![if physical == 2 { 0xff } else { 0 }]
        } else if physical == 2 {
            vec![0x80, 0xfe, 0xff, 0]
        } else {
            vec![0x7f, 1, 0]
        };
        new.extend_from_slice(&(((data as u32) << 8) | 3).to_be_bytes());
        wires.push(new);
        wires.sort_unstable();
        let mut active = Vec::new();
        let mut bitmap = [0; 453];
        for wire in wires {
            active.extend_from_slice(&wire);
            let end = active.len();
            assert!(end <= 3616);
            bitmap[end / 8] |= 1 << (end % 8);
        }
        let base = root * 4096;
        after[base + 2..base + 4]
            .copy_from_slice(&u16::try_from(3616 - active.len()).unwrap().to_le_bytes());
        after[base + 27..base + 480].copy_from_slice(&bitmap);
        after[base + 480..base + 480 + active.len()].copy_from_slice(&active);
        let history = if physical == 0 {
            4_u32
        } else if recipe == "insert-empty" {
            3
        } else {
            2
        };
        let at = td * 4096 + 67 + physical * 12;
        after[at..at + 4].copy_from_slice(&history.to_le_bytes());
    }
    (after, states)
}

fn inspect(
    raw: &[u8],
    table: &str,
    td: u32,
    data: u32,
    version: u8,
    states: &[(usize, Vec<Value>)],
) {
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
        let order = entries(raw, root as usize, data as usize)
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
    let properties =
        inspect_table_properties(&mut image.reader(), SCOPE, table, &mut budget(), &|| false)
            .unwrap();
    crate::properties::framing::validate(&properties, &mut budget(), &|| false).unwrap();
    let group = properties
        .groups()
        .iter()
        .find(|group| group.kind() == PropertyGroupKind::Column && group.name() == "KeyValue")
        .unwrap();
    let property = group.get("AllowZeroLength").unwrap();
    assert_eq!(property.native_type(), 1);
    assert_eq!(property.boolean().unwrap(), table.ends_with("Allow"));
    assert_eq!(
        property.raw_value(),
        &[if table.ends_with("Allow") { 255 } else { 0 }]
    );
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
        let (start, end) = frame(raw, data as usize, *slot);
        assert_eq!(record.reference, (data << 8) | *slot as u32);
        assert_eq!(record.offset, start);
        assert_eq!(record.data, raw[start..end]);
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires14 independent explicit-AZL original/literal views; no mutation calls"]
fn explicit_azl_originals_and_literal_inserts_have_complete_property_and_action_source() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_AZL_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        hash(&fs::read(directory.join("manifest.json")).unwrap()),
        "d1a0b4a2420d666b6fff24a16a49842a93555be80f9c33fa4eebac908f543219"
    );
    let mut views = Vec::new();
    for (id, filename, length, digest) in [
        (
            "h5-original",
            "ExplicitAZL-V2016-original.accdb",
            483328,
            "3dd7016d92d88987d0b923b0d5f5b8a29bb1ca1b285a1ca43eac7938637cccb5",
        ),
        (
            "h5-textlegacyallow-insert-empty",
            "h5-textlegacyallow-insert-empty.accdb",
            483328,
            "612a287bc0cacd5a13dfb2daabdd7e5d655071393c1a2bf518cd554070219170",
        ),
        (
            "h5-textlegacyallow-insert-null",
            "h5-textlegacyallow-insert-null.accdb",
            483328,
            "818a24157f671e7db45db46160563aa374ee073d5568772db5fb02032d64bc11",
        ),
        (
            "h5-textlegacydeny-insert-null",
            "h5-textlegacydeny-insert-null.accdb",
            483328,
            "33e4a690d3cb33f20c3036a7d229a4ec95b9219c451b0f5fcd8bc44db3d729b5",
        ),
        (
            "h5-textmodernallow-insert-empty",
            "h5-textmodernallow-insert-empty.accdb",
            483328,
            "70bca2e7bff7d05f6867a4dff274d03f065cf145fe1c7b60dc63b35ca373cd51",
        ),
        (
            "h5-textmodernallow-insert-null",
            "h5-textmodernallow-insert-null.accdb",
            483328,
            "66dead39b327cbc0fad6deeb57085ff842ab91cd058686c1ed7de5428a441092",
        ),
        (
            "h5-textmoderndeny-insert-null",
            "h5-textmoderndeny-insert-null.accdb",
            483328,
            "d19eb1493b0ee54846182cde2b5c4abb8fd020378455c5ace2c83893686be0be",
        ),
        (
            "h6-original",
            "ExplicitAZL-V2019-original.accdb",
            552960,
            "f9b6410e1408a4e8207450a13e46274f71ae7a6e9d6d55ca4ec81d5eb11d23e9",
        ),
        (
            "h6-textlegacyallow-insert-empty",
            "h6-textlegacyallow-insert-empty.accdb",
            552960,
            "0fbd047aadd492d3d2af97e9fc02607a6563231459a7d072f79c3a8ad2b37b86",
        ),
        (
            "h6-textlegacyallow-insert-null",
            "h6-textlegacyallow-insert-null.accdb",
            552960,
            "779cd57e574d31783db9ceec8abc26fd41b6e8f2c44cff11a8f3f867dc3b5e64",
        ),
        (
            "h6-textlegacydeny-insert-null",
            "h6-textlegacydeny-insert-null.accdb",
            552960,
            "0adf3146f594e08aafa86550af384e0edfeb053cc1b5d103c46828b825b85541",
        ),
        (
            "h6-textmodernallow-insert-empty",
            "h6-textmodernallow-insert-empty.accdb",
            552960,
            "1decb35c89d07149c7d50b2ce9af0c1725fbe5f52fcd1138164de5993cae4e1b",
        ),
        (
            "h6-textmodernallow-insert-null",
            "h6-textmodernallow-insert-null.accdb",
            552960,
            "73a505ec088a5dd70a27d9c688750ffdbefd4e0ca6d4f5faa79fda4f33b56a43",
        ),
        (
            "h6-textmoderndeny-insert-null",
            "h6-textmoderndeny-insert-null.accdb",
            552960,
            "cac76899d225d761f5bb0cbe97c52af74c62281410e333ff5978a3fc5abdc5e0",
        ),
    ] {
        let raw = fs::read(directory.join(filename)).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(hash(&raw), digest);
        views.push((id, raw));
    }
    let mut inspected = BTreeSet::new();
    let mut contexts = 0;
    for (header, table, td, data, version, recipe, identity) in [
        (
            5,
            "TextLegacyAllow",
            89,
            94,
            0,
            "insert-empty",
            "h5-textlegacyallow-insert-empty",
        ),
        (
            5,
            "TextLegacyAllow",
            89,
            94,
            0,
            "insert-null",
            "h5-textlegacyallow-insert-null",
        ),
        (
            5,
            "TextLegacyAllow",
            89,
            94,
            0,
            "empty-insert",
            "h5-original",
        ),
        (
            5,
            "TextLegacyDeny",
            96,
            101,
            0,
            "insert-null",
            "h5-textlegacydeny-insert-null",
        ),
        (
            5,
            "TextLegacyDeny",
            96,
            101,
            0,
            "empty-insert",
            "h5-original",
        ),
        (
            5,
            "TextModernAllow",
            103,
            109,
            1,
            "insert-empty",
            "h5-textmodernallow-insert-empty",
        ),
        (
            5,
            "TextModernAllow",
            103,
            109,
            1,
            "insert-null",
            "h5-textmodernallow-insert-null",
        ),
        (
            5,
            "TextModernAllow",
            103,
            109,
            1,
            "empty-insert",
            "h5-original",
        ),
        (
            5,
            "TextModernDeny",
            111,
            116,
            1,
            "insert-null",
            "h5-textmoderndeny-insert-null",
        ),
        (
            5,
            "TextModernDeny",
            111,
            116,
            1,
            "empty-insert",
            "h5-original",
        ),
        (
            6,
            "TextLegacyAllow",
            106,
            112,
            0,
            "insert-empty",
            "h6-textlegacyallow-insert-empty",
        ),
        (
            6,
            "TextLegacyAllow",
            106,
            112,
            0,
            "insert-null",
            "h6-textlegacyallow-insert-null",
        ),
        (
            6,
            "TextLegacyAllow",
            106,
            112,
            0,
            "empty-insert",
            "h6-original",
        ),
        (
            6,
            "TextLegacyDeny",
            114,
            119,
            0,
            "insert-null",
            "h6-textlegacydeny-insert-null",
        ),
        (
            6,
            "TextLegacyDeny",
            114,
            119,
            0,
            "empty-insert",
            "h6-original",
        ),
        (
            6,
            "TextModernAllow",
            121,
            126,
            1,
            "insert-empty",
            "h6-textmodernallow-insert-empty",
        ),
        (
            6,
            "TextModernAllow",
            121,
            126,
            1,
            "insert-null",
            "h6-textmodernallow-insert-null",
        ),
        (
            6,
            "TextModernAllow",
            121,
            126,
            1,
            "empty-insert",
            "h6-original",
        ),
        (
            6,
            "TextModernDeny",
            128,
            133,
            1,
            "insert-null",
            "h6-textmoderndeny-insert-null",
        ),
        (
            6,
            "TextModernDeny",
            128,
            133,
            1,
            "empty-insert",
            "h6-original",
        ),
    ] {
        let original = format!("h{header}-original");
        let before = &views.iter().find(|(id, _)| *id == original).unwrap().1;
        if inspected.insert((table, original.clone())) {
            let states = rows().into_iter().enumerate().collect::<Vec<_>>();
            inspect(before, table, td, data, version, &states);
        }
        let (expected, states) = literal(before, td as usize, data as usize, recipe);
        let after = &views.iter().find(|(id, _)| *id == identity).unwrap().1;
        assert_eq!(&expected, after, "{header}/{table}/{recipe}");
        if inspected.insert((table, identity.to_owned())) {
            inspect(after, table, td, data, version, &states);
        }
        contexts += 1;
    }
    assert_eq!(contexts, 20);
    assert_eq!(inspected.len(), 20);
    assert_eq!(views.len(), 14);
}
