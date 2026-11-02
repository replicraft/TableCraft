//! Immutable independently authored Text cardinality source corpus.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone)]
pub struct Case {
    pub header: u8,
    pub table: &'static str,
    pub action: &'static str,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
}

pub fn original_rows() -> Vec<Vec<Value>> {
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

impl Case {
    pub fn insert_values(&self) -> Vec<Vec<Value>> {
        assert_eq!(self.action, "insert");
        supplied(self.recipe)
    }

    pub fn deleted_slots(&self) -> Vec<usize> {
        assert_eq!(self.action, "delete");
        deleted(self.recipe)
    }

    pub fn changed_rows(&self) -> usize {
        if self.action == "insert" {
            self.insert_values().len()
        } else {
            self.deleted_slots().len()
        }
    }

    pub fn final_rows(&self) -> Vec<Vec<Value>> {
        let mut rows = original_rows();
        if self.action == "insert" {
            rows.extend(self.insert_values());
        } else {
            let slots = self.deleted_slots();
            rows = rows
                .into_iter()
                .enumerate()
                .filter(|(ordinal, _)| !slots.contains(ordinal))
                .map(|(_, row)| row)
                .collect();
        }
        rows
    }

    /// Actual syntax-supported request only; multirow/empty INSERT stays native.
    pub fn sql(&self) -> Option<String> {
        if self.action == "insert" {
            let mut rows = self.insert_values();
            if rows.len() != 1 {
                return None;
            }
            let row = rows.remove(0);
            let Value::Int32(identity) = row[0] else {
                panic!("manual source ID")
            };
            let value = match &row[1] {
                Value::Null => "NULL".to_owned(),
                Value::Text(value) => format!("'{}'", value.replace('\'', "''")),
                _ => panic!("source Text/NULL"),
            };
            Some(format!(
                "INSERT INTO [{}] ([KeyValue],[ID]) VALUES ({value},{identity})",
                self.table
            ))
        } else {
            let selected = self.deleted_slots();
            let condition = deletion_predicate(&selected);
            Some(format!("DELETE FROM [{}] WHERE {condition}", self.table))
        }
    }

    pub fn witnesses(&self) -> Vec<(usize, Vec<u8>)> {
        self.deleted_slots()
            .into_iter()
            .map(|slot| {
                let page = &self.before[self.data as usize * 4096..(self.data as usize + 1) * 4096];
                let start = usize::from(u16::from_le_bytes(
                    page[14 + slot * 2..16 + slot * 2].try_into().unwrap(),
                ));
                let end = if slot == 0 {
                    4096
                } else {
                    usize::from(u16::from_le_bytes(
                        page[12 + slot * 2..14 + slot * 2].try_into().unwrap(),
                    ))
                };
                assert!(start < end && end <= 4096);
                (slot, page[start..end].to_vec())
            })
            .collect()
    }
}

/// Preserve every selected ID and its order within the default binding depth.
fn deletion_predicate(selected: &[usize]) -> String {
    match selected {
        [] => "[ID]=-1".to_owned(),
        [slot] => format!("[ID]={}", slot + 1),
        _ => {
            let (left, right) = selected.split_at(selected.len() / 2);
            format!(
                "({} OR {})",
                deletion_predicate(left),
                deletion_predicate(right)
            )
        }
    }
}

/// Verify all54 whole images before returning any operation case.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_CARDINALITY_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    verify(
        &fs::read(directory.join("manifest.json")).unwrap(),
        "afe05784c2493bbcc0f2803c132528b77424da6c53b8c72f6b26e7cdfd4e8654",
    );
    let mut images = Vec::new();
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
        verify(&raw, digest);
        images.push((id, raw));
    }
    let mut cases = Vec::new();
    for (header, table, definition, action, recipe, identity) in [
        (
            5,
            "TextLegacyLeaf",
            89,
            "insert",
            "insert-null",
            "h5-textlegacyleaf-insert-null",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "insert",
            "insert-empty",
            "h5-textlegacyleaf-insert-empty",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "insert",
            "insert-a",
            "h5-textlegacyleaf-insert-a",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "insert",
            "insert-accent",
            "h5-textlegacyleaf-insert-accent",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "insert",
            "batch-a-null",
            "h5-textlegacyleaf-batch-a-null",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "insert",
            "batch-null-a",
            "h5-textlegacyleaf-batch-null-a",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "insert",
            "empty-insert",
            "h5-original",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "delete-null",
            "h5-textlegacyleaf-delete-null",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "delete-empty",
            "h5-textlegacyleaf-delete-empty",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "delete-present",
            "h5-textlegacyleaf-delete-present",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "delete-case-pair",
            "h5-textlegacyleaf-delete-case-pair",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "delete-accent-pair",
            "h5-textlegacyleaf-delete-accent-pair",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "delete-exteriors",
            "h5-textlegacyleaf-delete-exteriors",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "delete-exteriors-reverse",
            "h5-textlegacyleaf-delete-exteriors",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "delete-all",
            "h5-textlegacyleaf-delete-all",
        ),
        (
            5,
            "TextLegacyLeaf",
            89,
            "delete",
            "empty-delete",
            "h5-original",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "insert",
            "insert-null",
            "h5-textmodernleaf-insert-null",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "insert",
            "insert-empty",
            "h5-textmodernleaf-insert-empty",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "insert",
            "insert-a",
            "h5-textmodernleaf-insert-a",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "insert",
            "insert-accent",
            "h5-textmodernleaf-insert-accent",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "insert",
            "batch-a-null",
            "h5-textmodernleaf-batch-a-null",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "insert",
            "batch-null-a",
            "h5-textmodernleaf-batch-null-a",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "insert",
            "empty-insert",
            "h5-original",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "delete-null",
            "h5-textmodernleaf-delete-null",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "delete-empty",
            "h5-textmodernleaf-delete-empty",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "delete-present",
            "h5-textmodernleaf-delete-present",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "delete-case-pair",
            "h5-textmodernleaf-delete-case-pair",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "delete-accent-pair",
            "h5-textmodernleaf-delete-accent-pair",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "delete-exteriors",
            "h5-textmodernleaf-delete-exteriors",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "delete-exteriors-reverse",
            "h5-textmodernleaf-delete-exteriors",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "delete-all",
            "h5-textmodernleaf-delete-all",
        ),
        (
            5,
            "TextModernLeaf",
            96,
            "delete",
            "empty-delete",
            "h5-original",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "insert",
            "insert-null",
            "h6-textlegacyleaf-insert-null",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "insert",
            "insert-empty",
            "h6-textlegacyleaf-insert-empty",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "insert",
            "insert-a",
            "h6-textlegacyleaf-insert-a",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "insert",
            "insert-accent",
            "h6-textlegacyleaf-insert-accent",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "insert",
            "batch-a-null",
            "h6-textlegacyleaf-batch-a-null",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "insert",
            "batch-null-a",
            "h6-textlegacyleaf-batch-null-a",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "insert",
            "empty-insert",
            "h6-original",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "delete-null",
            "h6-textlegacyleaf-delete-null",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "delete-empty",
            "h6-textlegacyleaf-delete-empty",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "delete-present",
            "h6-textlegacyleaf-delete-present",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "delete-case-pair",
            "h6-textlegacyleaf-delete-case-pair",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "delete-accent-pair",
            "h6-textlegacyleaf-delete-accent-pair",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "delete-exteriors",
            "h6-textlegacyleaf-delete-exteriors",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "delete-exteriors-reverse",
            "h6-textlegacyleaf-delete-exteriors",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "delete-all",
            "h6-textlegacyleaf-delete-all",
        ),
        (
            6,
            "TextLegacyLeaf",
            106,
            "delete",
            "empty-delete",
            "h6-original",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "insert",
            "insert-null",
            "h6-textmodernleaf-insert-null",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "insert",
            "insert-empty",
            "h6-textmodernleaf-insert-empty",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "insert",
            "insert-a",
            "h6-textmodernleaf-insert-a",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "insert",
            "insert-accent",
            "h6-textmodernleaf-insert-accent",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "insert",
            "batch-a-null",
            "h6-textmodernleaf-batch-a-null",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "insert",
            "batch-null-a",
            "h6-textmodernleaf-batch-null-a",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "insert",
            "empty-insert",
            "h6-original",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "delete-null",
            "h6-textmodernleaf-delete-null",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "delete-empty",
            "h6-textmodernleaf-delete-empty",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "delete-present",
            "h6-textmodernleaf-delete-present",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "delete-case-pair",
            "h6-textmodernleaf-delete-case-pair",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "delete-accent-pair",
            "h6-textmodernleaf-delete-accent-pair",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "delete-exteriors",
            "h6-textmodernleaf-delete-exteriors",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "delete-exteriors-reverse",
            "h6-textmodernleaf-delete-exteriors",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "delete-all",
            "h6-textmodernleaf-delete-all",
        ),
        (
            6,
            "TextModernLeaf",
            113,
            "delete",
            "empty-delete",
            "h6-original",
        ),
    ] {
        let original = format!("h{header}-original");
        let before = images
            .iter()
            .find(|(id, _)| *id == original)
            .unwrap()
            .1
            .clone();
        let after = images
            .iter()
            .find(|(id, _)| *id == identity)
            .unwrap()
            .1
            .clone();
        cases.push(Case {
            header,
            table,
            action,
            recipe,
            before,
            after,
            definition,
            data: definition + 5,
            roots: [definition + 2, definition + 3, definition + 4],
        });
    }
    assert_eq!(cases.len(), 64);
    cases
}
