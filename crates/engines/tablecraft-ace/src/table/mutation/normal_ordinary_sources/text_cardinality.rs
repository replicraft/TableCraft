//! Literal Text cardinality source checks; no mutation planner or serializer.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_indexes,
    inspect_table_schema, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(228);

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

fn supplied(recipe: &str) -> Vec<Vec<Value>> {
    let row = |id, text: Option<&str>| {
        vec![
            Value::Int32(id),
            text.map_or(Value::Null, |value| Value::Text(value.to_owned())),
        ]
    };
    match recipe {
        "insert-null" => vec![row(25, None)],
        "insert-empty" => vec![row(25, Some(""))],
        "insert-a" => vec![row(25, Some("a"))],
        "insert-accent" => vec![row(25, Some("é"))],
        "batch-a-null" => vec![row(25, Some("a")), row(26, None)],
        "batch-null-a" => vec![row(26, None), row(25, Some("a"))],
        "empty-insert" => Vec::new(),
        _ => panic!("closed INSERT recipe"),
    }
}

fn deleted(recipe: &str) -> Vec<usize> {
    match recipe {
        "delete-null" => vec![0],
        "delete-empty" => vec![2],
        "delete-present" => vec![4],
        "delete-case-pair" => vec![4, 6],
        "delete-accent-pair" => vec![8, 10],
        "delete-exteriors" => vec![0, 23],
        "delete-exteriors-reverse" => vec![23, 0],
        "delete-all" => (0..24).collect(),
        "empty-delete" => Vec::new(),
        _ => panic!("closed DELETE recipe"),
    }
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
    action: &str,
    recipe: &str,
) -> (Vec<u8>, Vec<(usize, Vec<Value>)>) {
    let original = rows();
    let data = td + 5;
    let mut after = before.to_vec();
    let mut states = original.iter().cloned().enumerate().collect::<Vec<_>>();
    for (slot, row) in original.iter().enumerate() {
        let (start, end) = frame(before, data, slot);
        assert_eq!(before[start..end], encoded_record(row));
    }
    if action == "insert" {
        let mut frontier = 2672;
        for row in supplied(recipe) {
            let slot = states.len();
            let record = encoded_record(&row);
            frontier -= record.len();
            after[data * 4096 + frontier..data * 4096 + frontier + record.len()]
                .copy_from_slice(&record);
            after[data * 4096 + 14 + slot * 2..data * 4096 + 16 + slot * 2]
                .copy_from_slice(&u16::try_from(frontier).unwrap().to_le_bytes());
            states.push((slot, row));
        }
        if states.len() != 24 {
            after[data * 4096 + 12..data * 4096 + 14]
                .copy_from_slice(&u16::try_from(states.len()).unwrap().to_le_bytes());
            after[data * 4096 + 2..data * 4096 + 4].copy_from_slice(
                &u16::try_from(frontier - 14 - states.len() * 2)
                    .unwrap()
                    .to_le_bytes(),
            );
            after[td * 4096 + 67..td * 4096 + 71]
                .copy_from_slice(&u32::try_from(states.len()).unwrap().to_le_bytes());
        }
    } else {
        assert_eq!(action, "delete");
        let slots = deleted(recipe);
        for slot in &slots {
            let offset = data * 4096 + 14 + slot * 2;
            let word = u16::try_from(u16_at(before, offset)).unwrap() | 0xc000;
            after[offset..offset + 2].copy_from_slice(&word.to_le_bytes());
        }
        states.retain(|(slot, _)| !slots.contains(slot));
    }
    if states.len() != 24 {
        after[td * 4096 + 16..td * 4096 + 20]
            .copy_from_slice(&u32::try_from(states.len()).unwrap().to_le_bytes());
        for physical in 0..3 {
            let root = td + 2 + physical;
            let observed = entries(before, root, data);
            assert_eq!(observed.len(), 24);
            let mut final_wires = Vec::new();
            for (slot, row) in &states {
                let mut component = if physical == 0 {
                    let Value::Int32(id) = row[0] else {
                        panic!("I32 ID")
                    };
                    let mut value = vec![0x7f];
                    value.extend_from_slice(&((id as u32) ^ 0x80000000).to_be_bytes());
                    value
                } else {
                    let original_slot = original
                        .iter()
                        .position(|prior| prior[1] == row[1])
                        .unwrap();
                    observed
                        .iter()
                        .find(|(_, slot)| *slot == original_slot)
                        .unwrap()
                        .0
                        .clone()
                };
                component.extend_from_slice(&(((data as u32) << 8) | *slot as u32).to_be_bytes());
                final_wires.push(component);
            }
            final_wires.sort_unstable();
            let mut active = Vec::new();
            let mut bitmap = [0; 453];
            for wire in final_wires {
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
        }
    }
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
#[ignore = "requires54 independently authored Text cardinality views; read-only source checks"]
fn fitting_text_cardinality_views_have_exact_raw_and_complete_action_source() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_CARDINALITY_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        hash(&fs::read(directory.join("manifest.json")).unwrap()),
        "afe05784c2493bbcc0f2803c132528b77424da6c53b8c72f6b26e7cdfd4e8654"
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
            "h5-textlegacyleaf-insert-null",
            "h5-textlegacyleaf-insert-null.accdb",
            1163264,
            "23dd6e36773547622466c9b897754251b613000f33cf4147dca057e85698057d",
        ),
        (
            "h5-textlegacyleaf-insert-empty",
            "h5-textlegacyleaf-insert-empty.accdb",
            1163264,
            "fb96e63a87b2af3c257d9c8ff06041f554c682f7d1b077ec3605033cec799b63",
        ),
        (
            "h5-textlegacyleaf-insert-a",
            "h5-textlegacyleaf-insert-a.accdb",
            1163264,
            "717ea8669c462ce53b7a4ce15a0cc48a6438821eb6e480d53dc41682c0e065a5",
        ),
        (
            "h5-textlegacyleaf-insert-accent",
            "h5-textlegacyleaf-insert-accent.accdb",
            1163264,
            "c60a69b9169d97f4140e4e5e43529e7d5c1f1c322572e949952a1c1afbdf751a",
        ),
        (
            "h5-textlegacyleaf-batch-a-null",
            "h5-textlegacyleaf-batch-a-null.accdb",
            1163264,
            "aa756d2c4f6d7375d6c0f4eb93814439c19143f994ed89733195160c8318910c",
        ),
        (
            "h5-textlegacyleaf-batch-null-a",
            "h5-textlegacyleaf-batch-null-a.accdb",
            1163264,
            "5a8ab8ed35e7e9dfd096fc708edb1f34ce48079f15700f7ad1ea533347365fd9",
        ),
        (
            "h5-textlegacyleaf-delete-null",
            "h5-textlegacyleaf-delete-null.accdb",
            1163264,
            "5b6cbf37fae0a04435230dd632519d59bc52e9e22702a2e166988c9ebb8cf72a",
        ),
        (
            "h5-textlegacyleaf-delete-empty",
            "h5-textlegacyleaf-delete-empty.accdb",
            1163264,
            "268de3c4e5dfebeaa62b5d31bd9f9e097a80234dcbbc99523071aad48f8ec486",
        ),
        (
            "h5-textlegacyleaf-delete-present",
            "h5-textlegacyleaf-delete-present.accdb",
            1163264,
            "beb15fe5b7312399b0fd15ee67d0c020adc7e5aa5fbe219225cacd6310a90613",
        ),
        (
            "h5-textlegacyleaf-delete-case-pair",
            "h5-textlegacyleaf-delete-case-pair.accdb",
            1163264,
            "fdb5a0a242ae88a013bac56f536f60e3e44a223e530736343a5d32e6854a48fa",
        ),
        (
            "h5-textlegacyleaf-delete-accent-pair",
            "h5-textlegacyleaf-delete-accent-pair.accdb",
            1163264,
            "4576abd0d1218b86fa9c61e1c1216a5ffb1e82878d31ecbe678fd18c9b1147e5",
        ),
        (
            "h5-textlegacyleaf-delete-exteriors",
            "h5-textlegacyleaf-delete-exteriors.accdb",
            1163264,
            "2f6a6ad4cfb875a325b5da7c89b61a79457dd3441c9805bf895f13d6208a84b8",
        ),
        (
            "h5-textlegacyleaf-delete-all",
            "h5-textlegacyleaf-delete-all.accdb",
            1163264,
            "0943547cfff4c65ff61f9af379f01c49e379f2a867ce6fb922cfad8a90b85434",
        ),
        (
            "h5-textmodernleaf-insert-null",
            "h5-textmodernleaf-insert-null.accdb",
            1163264,
            "d25c5b84962554f168dfa424bfeb1d69406fc8639f2ae7f5eee7707438f4fb24",
        ),
        (
            "h5-textmodernleaf-insert-empty",
            "h5-textmodernleaf-insert-empty.accdb",
            1163264,
            "77f1c69bebe5d3c57b60b6ae677946cf7fe97744addbefedfe1cdd057a1097c5",
        ),
        (
            "h5-textmodernleaf-insert-a",
            "h5-textmodernleaf-insert-a.accdb",
            1163264,
            "d2c071262a93f864ac8640a5760bb99a7fdf6b55ddc2f40b11b7c2f0bed8c349",
        ),
        (
            "h5-textmodernleaf-insert-accent",
            "h5-textmodernleaf-insert-accent.accdb",
            1163264,
            "ae3917457304befd8b050dfd93c95008ad946f38055b8ff98bd9213460d811ba",
        ),
        (
            "h5-textmodernleaf-batch-a-null",
            "h5-textmodernleaf-batch-a-null.accdb",
            1163264,
            "6954d61e32fafd194634e31e7d58ecc2c3773b02aa6cf38ea42841a5be6dd694",
        ),
        (
            "h5-textmodernleaf-batch-null-a",
            "h5-textmodernleaf-batch-null-a.accdb",
            1163264,
            "c2685d148c5d24aafd085c8a3a0b2d8a68e4249d6c62444e3242e36383e51ab1",
        ),
        (
            "h5-textmodernleaf-delete-null",
            "h5-textmodernleaf-delete-null.accdb",
            1163264,
            "7bb84507a3536bfb73e3e61ce1b84b79b0e32b93298f8a12eac28cc28e1b8fe4",
        ),
        (
            "h5-textmodernleaf-delete-empty",
            "h5-textmodernleaf-delete-empty.accdb",
            1163264,
            "6495783e3c4c424fd3b4f2526388dcc46f8d0bb5fd158880544f7f061796b949",
        ),
        (
            "h5-textmodernleaf-delete-present",
            "h5-textmodernleaf-delete-present.accdb",
            1163264,
            "46cba23a3052eece72e80f0c095e358fd55d233b3df9da07fb8eb16d22015da9",
        ),
        (
            "h5-textmodernleaf-delete-case-pair",
            "h5-textmodernleaf-delete-case-pair.accdb",
            1163264,
            "32efdd1d889b601988dc86630d31881a2d5a6d6232fd66cdf4ba7e576c688868",
        ),
        (
            "h5-textmodernleaf-delete-accent-pair",
            "h5-textmodernleaf-delete-accent-pair.accdb",
            1163264,
            "eccaf2ee6b5162ca8527e7c6f529786f4c327726dc8627a05cc23e7a187f1437",
        ),
        (
            "h5-textmodernleaf-delete-exteriors",
            "h5-textmodernleaf-delete-exteriors.accdb",
            1163264,
            "8bddfa616ee21274468c0d2d5baf35fbd27807dbc6ba0c5d3a3a496e64e81fab",
        ),
        (
            "h5-textmodernleaf-delete-all",
            "h5-textmodernleaf-delete-all.accdb",
            1163264,
            "2c0fc14d330e0430fe162f8c39032e8592eee16e25bc8a1ef91a4fdecd5dd035",
        ),
        (
            "h6-original",
            "Encoded-V2019-before.accdb",
            1236992,
            "636e137ee3a1d51f6525072de87963cb33a03b01a253be1d2124cf6bb4746279",
        ),
        (
            "h6-textlegacyleaf-insert-null",
            "h6-textlegacyleaf-insert-null.accdb",
            1236992,
            "a54f2bd61747c16cfe596941a6fb76ddb3a26f1d2cb0062577851a7e387125a6",
        ),
        (
            "h6-textlegacyleaf-insert-empty",
            "h6-textlegacyleaf-insert-empty.accdb",
            1236992,
            "819af627fcfe1b53284c688c5bfb6cc24bc6164212f6daa56e5d2a7f1f00b887",
        ),
        (
            "h6-textlegacyleaf-insert-a",
            "h6-textlegacyleaf-insert-a.accdb",
            1236992,
            "2913d801c7f5b3a38270b341bcc496596a66026fa999a1340012e32213cf72f1",
        ),
        (
            "h6-textlegacyleaf-insert-accent",
            "h6-textlegacyleaf-insert-accent.accdb",
            1236992,
            "2427b24c718f26438a540bbc164765e2d50fdb1e1fd12645824f8d7fa1867cd3",
        ),
        (
            "h6-textlegacyleaf-batch-a-null",
            "h6-textlegacyleaf-batch-a-null.accdb",
            1236992,
            "ce459d69fce1bef2df8216a3838b140acadf0d3ab2e602d38112e36207f94b24",
        ),
        (
            "h6-textlegacyleaf-batch-null-a",
            "h6-textlegacyleaf-batch-null-a.accdb",
            1236992,
            "8bf544f679ff4a4453b3678d27d0ca7188dc9de3f5cc5b2489ff5cd0acf51ac3",
        ),
        (
            "h6-textlegacyleaf-delete-null",
            "h6-textlegacyleaf-delete-null.accdb",
            1236992,
            "1c4d411af5be0481de453fbde02555e6ef387b45df802196456e55d23c1ce662",
        ),
        (
            "h6-textlegacyleaf-delete-empty",
            "h6-textlegacyleaf-delete-empty.accdb",
            1236992,
            "27cee17db4ec2212ef04c9043e4dc55af63c8220c91bc0de05a000dec7928d9f",
        ),
        (
            "h6-textlegacyleaf-delete-present",
            "h6-textlegacyleaf-delete-present.accdb",
            1236992,
            "d48f67553ed866ca73782a4e9c9189a4b41002b97f574aa7e6ab4776cd339904",
        ),
        (
            "h6-textlegacyleaf-delete-case-pair",
            "h6-textlegacyleaf-delete-case-pair.accdb",
            1236992,
            "6e49a2972fa4d5fed45121c93b5a7cb6c7d31a6109220f6d45dff19564e5eccf",
        ),
        (
            "h6-textlegacyleaf-delete-accent-pair",
            "h6-textlegacyleaf-delete-accent-pair.accdb",
            1236992,
            "e6f1670662a04c2cdaabd3b5ffa43731aa12ebbac70ff1f424c27649dba232a5",
        ),
        (
            "h6-textlegacyleaf-delete-exteriors",
            "h6-textlegacyleaf-delete-exteriors.accdb",
            1236992,
            "8683ece2e3679c32bb56039263d9f6e3b6c358f1eefc08e60e18a3ab35fb6ca9",
        ),
        (
            "h6-textlegacyleaf-delete-all",
            "h6-textlegacyleaf-delete-all.accdb",
            1236992,
            "3113dc7fabaa28867d02b8bfa6af43555569f413be10670d346ab6e7d4939f31",
        ),
        (
            "h6-textmodernleaf-insert-null",
            "h6-textmodernleaf-insert-null.accdb",
            1236992,
            "0b9f289ce231132553ece4be570235cd6184a175f119b929066687cdf5778cd4",
        ),
        (
            "h6-textmodernleaf-insert-empty",
            "h6-textmodernleaf-insert-empty.accdb",
            1236992,
            "a0d3906d461bfac5e6d1a77c7a288fb66355fc0e6d1c03858158bda2eefdf8e2",
        ),
        (
            "h6-textmodernleaf-insert-a",
            "h6-textmodernleaf-insert-a.accdb",
            1236992,
            "ddbd4f0114a9c2aab98838989e3c4c5da17af0bf8138ac4a4ffa075ac99fcbb9",
        ),
        (
            "h6-textmodernleaf-insert-accent",
            "h6-textmodernleaf-insert-accent.accdb",
            1236992,
            "be27f89514a4de48cc369e53da7c4f93346818464d1b8c8817ee23ca40777bc2",
        ),
        (
            "h6-textmodernleaf-batch-a-null",
            "h6-textmodernleaf-batch-a-null.accdb",
            1236992,
            "e4862e266aeedec0cb873de93ae863ccefd9740b056e3e50fe68c5b120dc4f41",
        ),
        (
            "h6-textmodernleaf-batch-null-a",
            "h6-textmodernleaf-batch-null-a.accdb",
            1236992,
            "9ced3e724aeafd9142d6248588ea9eabe48cedb65621eca0df79baeb02421a63",
        ),
        (
            "h6-textmodernleaf-delete-null",
            "h6-textmodernleaf-delete-null.accdb",
            1236992,
            "f9b3d7d338d7839c2bbf98f0c2efff63ddce10ec87594ca775d3f86cff6b5a3b",
        ),
        (
            "h6-textmodernleaf-delete-empty",
            "h6-textmodernleaf-delete-empty.accdb",
            1236992,
            "21e9e2c20f42fba8d55a6c06810b6f44104055ced7533dfc57ee5dfe55d5f31b",
        ),
        (
            "h6-textmodernleaf-delete-present",
            "h6-textmodernleaf-delete-present.accdb",
            1236992,
            "1c5ad94da00eb0bd89bed97c95e6abdc94cb8ce304b44efa608c49f3e093c405",
        ),
        (
            "h6-textmodernleaf-delete-case-pair",
            "h6-textmodernleaf-delete-case-pair.accdb",
            1236992,
            "e921eec1d4fbc6a30875850dcceac190c777e332db6656071252dec5323e0516",
        ),
        (
            "h6-textmodernleaf-delete-accent-pair",
            "h6-textmodernleaf-delete-accent-pair.accdb",
            1236992,
            "a007a20d60521aa5773157256bb291af6f37cf6140ecb8c55b13ed4092f81f3f",
        ),
        (
            "h6-textmodernleaf-delete-exteriors",
            "h6-textmodernleaf-delete-exteriors.accdb",
            1236992,
            "1ae0c9f2a8573d50d17eb0f271aafbfb116bbea91d233162cf5fa20295343a1c",
        ),
        (
            "h6-textmodernleaf-delete-all",
            "h6-textmodernleaf-delete-all.accdb",
            1236992,
            "a15c85c5d0cea29cdd6382f4470edad28cf8d317e8ed0078c8a4a186fd267f0e",
        ),
    ] {
        let raw = fs::read(directory.join(filename)).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(hash(&raw), digest);
        views.push((id, raw));
    }
    let mut inspected = BTreeSet::new();
    let mut reconstructions = 0;
    for (header, table, td, version, action, recipe, identity) in [
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "insert",
            "insert-null",
            "h5-textlegacyleaf-insert-null",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "insert",
            "insert-empty",
            "h5-textlegacyleaf-insert-empty",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "insert",
            "insert-a",
            "h5-textlegacyleaf-insert-a",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "insert",
            "insert-accent",
            "h5-textlegacyleaf-insert-accent",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "insert",
            "batch-a-null",
            "h5-textlegacyleaf-batch-a-null",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "insert",
            "batch-null-a",
            "h5-textlegacyleaf-batch-null-a",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "insert",
            "empty-insert",
            "h5-original",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "delete-null",
            "h5-textlegacyleaf-delete-null",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "delete-empty",
            "h5-textlegacyleaf-delete-empty",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "delete-present",
            "h5-textlegacyleaf-delete-present",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "delete-case-pair",
            "h5-textlegacyleaf-delete-case-pair",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "delete-accent-pair",
            "h5-textlegacyleaf-delete-accent-pair",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "delete-exteriors",
            "h5-textlegacyleaf-delete-exteriors",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "delete-exteriors-reverse",
            "h5-textlegacyleaf-delete-exteriors",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "delete-all",
            "h5-textlegacyleaf-delete-all",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            0,
            "delete",
            "empty-delete",
            "h5-original",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "insert",
            "insert-null",
            "h5-textmodernleaf-insert-null",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "insert",
            "insert-empty",
            "h5-textmodernleaf-insert-empty",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "insert",
            "insert-a",
            "h5-textmodernleaf-insert-a",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "insert",
            "insert-accent",
            "h5-textmodernleaf-insert-accent",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "insert",
            "batch-a-null",
            "h5-textmodernleaf-batch-a-null",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "insert",
            "batch-null-a",
            "h5-textmodernleaf-batch-null-a",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "insert",
            "empty-insert",
            "h5-original",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "delete-null",
            "h5-textmodernleaf-delete-null",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "delete-empty",
            "h5-textmodernleaf-delete-empty",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "delete-present",
            "h5-textmodernleaf-delete-present",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "delete-case-pair",
            "h5-textmodernleaf-delete-case-pair",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "delete-accent-pair",
            "h5-textmodernleaf-delete-accent-pair",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "delete-exteriors",
            "h5-textmodernleaf-delete-exteriors",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "delete-exteriors-reverse",
            "h5-textmodernleaf-delete-exteriors",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "delete-all",
            "h5-textmodernleaf-delete-all",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            1,
            "delete",
            "empty-delete",
            "h5-original",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "insert",
            "insert-null",
            "h6-textlegacyleaf-insert-null",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "insert",
            "insert-empty",
            "h6-textlegacyleaf-insert-empty",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "insert",
            "insert-a",
            "h6-textlegacyleaf-insert-a",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "insert",
            "insert-accent",
            "h6-textlegacyleaf-insert-accent",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "insert",
            "batch-a-null",
            "h6-textlegacyleaf-batch-a-null",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "insert",
            "batch-null-a",
            "h6-textlegacyleaf-batch-null-a",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "insert",
            "empty-insert",
            "h6-original",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "delete-null",
            "h6-textlegacyleaf-delete-null",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "delete-empty",
            "h6-textlegacyleaf-delete-empty",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "delete-present",
            "h6-textlegacyleaf-delete-present",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "delete-case-pair",
            "h6-textlegacyleaf-delete-case-pair",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "delete-accent-pair",
            "h6-textlegacyleaf-delete-accent-pair",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "delete-exteriors",
            "h6-textlegacyleaf-delete-exteriors",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "delete-exteriors-reverse",
            "h6-textlegacyleaf-delete-exteriors",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "delete-all",
            "h6-textlegacyleaf-delete-all",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            0,
            "delete",
            "empty-delete",
            "h6-original",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "insert",
            "insert-null",
            "h6-textmodernleaf-insert-null",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "insert",
            "insert-empty",
            "h6-textmodernleaf-insert-empty",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "insert",
            "insert-a",
            "h6-textmodernleaf-insert-a",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "insert",
            "insert-accent",
            "h6-textmodernleaf-insert-accent",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "insert",
            "batch-a-null",
            "h6-textmodernleaf-batch-a-null",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "insert",
            "batch-null-a",
            "h6-textmodernleaf-batch-null-a",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "insert",
            "empty-insert",
            "h6-original",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "delete-null",
            "h6-textmodernleaf-delete-null",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "delete-empty",
            "h6-textmodernleaf-delete-empty",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "delete-present",
            "h6-textmodernleaf-delete-present",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "delete-case-pair",
            "h6-textmodernleaf-delete-case-pair",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "delete-accent-pair",
            "h6-textmodernleaf-delete-accent-pair",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "delete-exteriors",
            "h6-textmodernleaf-delete-exteriors",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "delete-exteriors-reverse",
            "h6-textmodernleaf-delete-exteriors",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "delete-all",
            "h6-textmodernleaf-delete-all",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            1,
            "delete",
            "empty-delete",
            "h6-original",
        ),
    ] {
        let original_id = format!("h{header}-original");
        let before = &views.iter().find(|(id, _)| *id == original_id).unwrap().1;
        if inspected.insert((table, original_id.clone())) {
            let states = rows().into_iter().enumerate().collect::<Vec<_>>();
            inspect(before, table, td, version, &states);
        }
        let (expected, states) = literal(before, td as usize, action, recipe);
        let after = &views.iter().find(|(id, _)| *id == identity).unwrap().1;
        assert_eq!(&expected, after, "{header}/{table}/{recipe}");
        if inspected.insert((table, identity.to_owned())) {
            inspect(after, table, td, version, &states);
        }
        reconstructions += 1;
    }
    assert_eq!(views.len(), 54);
    assert_eq!(reconstructions, 64);
    assert_eq!(inspected.len(), 56);
}
