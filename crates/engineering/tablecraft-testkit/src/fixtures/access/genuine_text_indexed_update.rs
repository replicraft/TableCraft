//! Independently constructed/read-qualified Text UPDATE images; std source loading.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub header: u8,
    pub table: &'static str,
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
        None,
        Some("カタカナ"),
    ];
    (0..24)
        .map(|row| {
            let position = row * 7 % 13;
            let value = if position == 11 {
                Value::Text("a".repeat(255))
            } else {
                values[position].map_or(Value::Null, |value| Value::Text(value.to_owned()))
            };
            vec![Value::Int32(row as i32 + 1), value]
        })
        .collect()
}

impl Case {
    pub fn updates(&self) -> Vec<(usize, Value, Value)> {
        let text = |value: &str| Value::Text(value.to_owned());
        let changes = match self.recipe {
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
            _ => panic!("closed Text source recipe"),
        };
        let original = original_rows();
        changes
            .into_iter()
            .map(|(row, value)| (row, original[row][1].clone(), value))
            .collect()
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        let mut rows = original_rows();
        for (row, _, value) in self.updates() {
            rows[row][1] = value;
        }
        rows
    }

    pub fn changed_rows(&self) -> usize {
        self.updates()
            .iter()
            .filter(|(_, before, after)| before != after)
            .count()
    }
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut images = Vec::new();
    for (id, file, length, digest) in [
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
        let bytes = fs::read(directory.join(file)).unwrap();
        assert_eq!(bytes.len(), length);
        verify(&bytes, digest);
        images.push((id, bytes));
    }
    let mut cases = Vec::new();
    for (header, table, recipe, view) in [
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
    ] {
        let original = if header == 5 {
            "h5-original"
        } else {
            "h6-original"
        };
        let definition = match (header, table) {
            (5, "TextLegacyLeaf") => 89,
            (5, "TextModernLeaf") => 96,
            (6, "TextLegacyLeaf") => 106,
            (6, "TextModernLeaf") => 113,
            _ => panic!("closed Text source table"),
        };
        cases.push(Case {
            header,
            table,
            recipe,
            before: images
                .iter()
                .find(|image| image.0 == original)
                .unwrap()
                .1
                .clone(),
            after: images
                .iter()
                .find(|image| image.0 == view)
                .unwrap()
                .1
                .clone(),
            definition,
            data: definition + 5,
            roots: [definition + 2, definition + 3, definition + 4],
        });
    }
    cases
}
