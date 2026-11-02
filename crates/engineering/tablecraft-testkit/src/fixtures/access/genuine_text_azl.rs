//! Independent explicit-AZL originals/outcomes, pinned before any producer.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone)]
pub struct Case {
    pub header: u8,
    pub table: &'static str,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Option<Vec<u8>>,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
    pub property_name_offset: usize,
}

pub fn original_rows() -> Vec<Vec<Value>> {
    vec![
        vec![Value::Int32(1), Value::Null],
        vec![Value::Int32(2), Value::Text("a".into())],
        vec![Value::Int32(3), Value::Text("A".into())],
    ]
}

impl Case {
    pub fn insert_values(&self) -> Vec<Vec<Value>> {
        match self.recipe {
            "empty-insert" => Vec::new(),
            "insert-empty" => vec![vec![Value::Int32(4), Value::Text(String::new())]],
            "insert-null" => vec![vec![Value::Int32(4), Value::Null]],
            _ => panic!("closed source recipe"),
        }
    }

    pub fn changed_rows(&self) -> usize {
        self.insert_values().len()
    }

    pub fn after_bytes(&self) -> &[u8] {
        self.after
            .as_deref()
            .expect("false/empty has no healthy AFTER")
    }

    pub fn final_rows(&self) -> Vec<Vec<Value>> {
        let mut rows = original_rows();
        rows.extend(self.insert_values());
        rows
    }

    pub fn sql(&self) -> Option<String> {
        match self.recipe {
            "empty-insert" => None,
            "insert-empty" => Some(format!(
                "INSERT INTO [{}] ([KeyValue],[ID]) VALUES ('',4)",
                self.table
            )),
            "insert-null" => Some(format!(
                "INSERT INTO [{}] ([KeyValue],[ID]) VALUES (NULL,4)",
                self.table
            )),
            _ => panic!("closed source recipe"),
        }
    }
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_AZL_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    verify(
        &fs::read(directory.join("manifest.json")).unwrap(),
        "d1a0b4a2420d666b6fff24a16a49842a93555be80f9c33fa4eebac908f543219",
    );
    let mut images = Vec::new();
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
        verify(&raw, digest);
        images.push((id, raw));
    }
    let mut cases = Vec::new();
    for (header, table, recipe, definition, data, expected) in [
        (
            5,
            "TextLegacyAllow",
            "insert-empty",
            89,
            94,
            Some("h5-textlegacyallow-insert-empty"),
        ),
        (
            5,
            "TextLegacyAllow",
            "insert-null",
            89,
            94,
            Some("h5-textlegacyallow-insert-null"),
        ),
        (
            5,
            "TextLegacyAllow",
            "empty-insert",
            89,
            94,
            Some("h5-original"),
        ),
        (5, "TextLegacyDeny", "insert-empty", 96, 101, None),
        (
            5,
            "TextLegacyDeny",
            "insert-null",
            96,
            101,
            Some("h5-textlegacydeny-insert-null"),
        ),
        (
            5,
            "TextLegacyDeny",
            "empty-insert",
            96,
            101,
            Some("h5-original"),
        ),
        (
            5,
            "TextModernAllow",
            "insert-empty",
            103,
            109,
            Some("h5-textmodernallow-insert-empty"),
        ),
        (
            5,
            "TextModernAllow",
            "insert-null",
            103,
            109,
            Some("h5-textmodernallow-insert-null"),
        ),
        (
            5,
            "TextModernAllow",
            "empty-insert",
            103,
            109,
            Some("h5-original"),
        ),
        (5, "TextModernDeny", "insert-empty", 111, 116, None),
        (
            5,
            "TextModernDeny",
            "insert-null",
            111,
            116,
            Some("h5-textmoderndeny-insert-null"),
        ),
        (
            5,
            "TextModernDeny",
            "empty-insert",
            111,
            116,
            Some("h5-original"),
        ),
        (
            6,
            "TextLegacyAllow",
            "insert-empty",
            106,
            112,
            Some("h6-textlegacyallow-insert-empty"),
        ),
        (
            6,
            "TextLegacyAllow",
            "insert-null",
            106,
            112,
            Some("h6-textlegacyallow-insert-null"),
        ),
        (
            6,
            "TextLegacyAllow",
            "empty-insert",
            106,
            112,
            Some("h6-original"),
        ),
        (6, "TextLegacyDeny", "insert-empty", 114, 119, None),
        (
            6,
            "TextLegacyDeny",
            "insert-null",
            114,
            119,
            Some("h6-textlegacydeny-insert-null"),
        ),
        (
            6,
            "TextLegacyDeny",
            "empty-insert",
            114,
            119,
            Some("h6-original"),
        ),
        (
            6,
            "TextModernAllow",
            "insert-empty",
            121,
            126,
            Some("h6-textmodernallow-insert-empty"),
        ),
        (
            6,
            "TextModernAllow",
            "insert-null",
            121,
            126,
            Some("h6-textmodernallow-insert-null"),
        ),
        (
            6,
            "TextModernAllow",
            "empty-insert",
            121,
            126,
            Some("h6-original"),
        ),
        (6, "TextModernDeny", "insert-empty", 128, 133, None),
        (
            6,
            "TextModernDeny",
            "insert-null",
            128,
            133,
            Some("h6-textmoderndeny-insert-null"),
        ),
        (
            6,
            "TextModernDeny",
            "empty-insert",
            128,
            133,
            Some("h6-original"),
        ),
    ] {
        let original = format!("h{header}-original");
        let before = images
            .iter()
            .find(|(id, _)| *id == original)
            .unwrap()
            .1
            .clone();
        let after = expected.map(|expected| {
            images
                .iter()
                .find(|(id, _)| *id == expected)
                .unwrap()
                .1
                .clone()
        });
        let needle = "AllowZeroLength"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let positions = before
            .windows(needle.len())
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == needle.as_slice()).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 4);
        cases.push(Case {
            header,
            table,
            recipe,
            before,
            after,
            definition,
            data,
            roots: [definition + 2, definition + 3, definition + 4],
            property_name_offset: positions[0],
        });
    }
    assert_eq!(cases.len(), 24);
    assert_eq!(cases.iter().filter(|case| case.after.is_some()).count(), 20);
    cases
}
