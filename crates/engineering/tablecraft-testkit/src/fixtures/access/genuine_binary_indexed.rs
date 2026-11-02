//! Frozen independently authored Binary outcomes; generic/source gates precede writers.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

const ORIGINAL_KEYS: [Option<&str>; 13] = [
    None,
    Some(""),
    Some("00"),
    Some("ff"),
    Some("0000"),
    Some("0001"),
    Some("00010203040506"),
    Some("0001020304050607"),
    Some("000102030405060708"),
    Some("000102030405060708090a0b0c0d0e0f"),
    Some("0000ff00"),
    Some("ff00ff00ff00ff00"),
    Some("00"),
];

pub struct Case {
    pub header: u8,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
}

fn binary(hex: &str) -> Value {
    assert!(hex.len().is_multiple_of(2));
    let bytes = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let nibble = |byte| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("outside frozen hexadecimal source"),
            };
            (nibble(pair[0]) << 4) | nibble(pair[1])
        })
        .collect();
    Value::Binary(bytes)
}

fn row(id: i32, key: Option<&str>, marker: i32) -> Vec<Value> {
    vec![
        Value::Int32(id),
        key.map_or(Value::Null, binary),
        Value::Int32(marker),
    ]
}

pub fn original_rows() -> Vec<Vec<Value>> {
    ORIGINAL_KEYS
        .iter()
        .enumerate()
        .map(|(slot, key)| row(slot as i32 + 1, *key, 17 * (slot as i32 + 1)))
        .collect()
}

/// Complete original supplied carriers, in written order. Empty Binary is not NULL.
pub fn insert_values(recipe: &str) -> Vec<Vec<Value>> {
    match recipe {
        "insert-null" => vec![row(14, None, 238)],
        "insert-empty" => vec![row(14, Some(""), 238)],
        "insert-existing" => vec![row(14, Some("00"), 238)],
        "insert-seven" => vec![row(14, Some("000102030405ff"), 238)],
        "insert-nine" => vec![row(14, Some("ff00ff00ff00ff0000"), 238)],
        "insert-maximum" => vec![row(14, Some("000102030405060708090a0b0c0d0eff"), 238)],
        "batch" | "batch-reverse" => {
            let mut rows = vec![
                row(14, None, 238),
                row(15, Some(""), 255),
                row(16, Some("00"), 272),
                row(17, Some("ff00ff00ff00ff0000"), 289),
                row(18, Some("000102030405060708090a0b0c0d0eff"), 306),
            ];
            if recipe == "batch-reverse" {
                rows.reverse();
            }
            rows
        }
        _ => panic!("outside frozen positive Binary source law"),
    }
}

impl Case {
    pub fn rows(&self) -> Vec<Vec<Value>> {
        let mut rows = original_rows();
        rows.extend(insert_values(self.recipe));
        rows
    }
}

/// Verify both originals and all sixteen AFTER files before returning cases.
/// Hash implementation stays with the caller; no new fixture dependency.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_BINARY_INDEXED_SOURCE_DIR").unwrap());
    assert!(dir.is_absolute());
    let before5 = fs::read(dir.join("h5-binary-before.accdb")).unwrap();
    let before6 = fs::read(dir.join("h6-binary-before.accdb")).unwrap();
    verify(
        &before5,
        "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398",
    );
    verify(
        &before6,
        "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
    );
    let mut cases = Vec::new();
    for (header, recipe, filename, hash) in [
        (
            5_u8,
            "insert-null",
            "h5-binary-insert-null.accdb",
            "894790c770abf08e4154347432bf7f3bed690c13461ab175a67573e7ce9fe46a",
        ),
        (
            5_u8,
            "insert-empty",
            "h5-binary-insert-empty.accdb",
            "75fb2c2082b32ff725044fd6b4fa8c8dd837e02b255cc996e8315df449015ab5",
        ),
        (
            5_u8,
            "insert-existing",
            "h5-binary-insert-existing.accdb",
            "de3c70d4f72c71157a250126fd62ed0ca01e9bbe17a2942d837bc75e119e6bf0",
        ),
        (
            5_u8,
            "insert-seven",
            "h5-binary-insert-seven.accdb",
            "52bcbb2258fe64acfbb8be434bde07d0bccd682ae84171f1e7103d021af223ed",
        ),
        (
            5_u8,
            "insert-nine",
            "h5-binary-insert-nine.accdb",
            "c720094bd6cf26d1e69fe9dc84ce09b5d01ca283ca81a189c084af457e9dc531",
        ),
        (
            5_u8,
            "insert-maximum",
            "h5-binary-insert-maximum.accdb",
            "3da6358b4e5e45284eef54d44750180473a9c428569a46f42d46f1e2fc45bf0c",
        ),
        (
            5_u8,
            "batch",
            "h5-binary-batch.accdb",
            "9bc273e1fa042f693e5355b05e7ee16390be27f2ca25813d1e1160ad0d545d76",
        ),
        (
            5_u8,
            "batch-reverse",
            "h5-binary-batch-reverse.accdb",
            "365c324531c49cda303071a39fd3264c0383d436ccbd3061fb0b00f88787da08",
        ),
        (
            6_u8,
            "insert-null",
            "h6-binary-insert-null.accdb",
            "1cddf2a16f655874d37ecdd469b591f21e6fa228b4f89bd8e80f739ca733a8ad",
        ),
        (
            6_u8,
            "insert-empty",
            "h6-binary-insert-empty.accdb",
            "7481e9015336dea239802be05c686e2d50d3bb82c2a6e07245633f372d96448b",
        ),
        (
            6_u8,
            "insert-existing",
            "h6-binary-insert-existing.accdb",
            "64d2b14a20e49ba651f5e01ced5a7137affe3c320ac41bf0fe76016be5a8e62b",
        ),
        (
            6_u8,
            "insert-seven",
            "h6-binary-insert-seven.accdb",
            "67c7398d778130532a211c9a02ae5b618925aafccdc5d88f8c0ca00eff81afc3",
        ),
        (
            6_u8,
            "insert-nine",
            "h6-binary-insert-nine.accdb",
            "9a9cc3c078104ec1f09f44357f2138994e93461b27fbb1e13653fb465507cf9a",
        ),
        (
            6_u8,
            "insert-maximum",
            "h6-binary-insert-maximum.accdb",
            "c309714e6085c8557ae2deee14abcf0cb7674d68698a34b2388143e3d94bc684",
        ),
        (
            6_u8,
            "batch",
            "h6-binary-batch.accdb",
            "f9caec0ef0b46260b31fd6c9c0368476924a28033653103530169d7ea6a83141",
        ),
        (
            6_u8,
            "batch-reverse",
            "h6-binary-batch-reverse.accdb",
            "10ed68d96a9c5cbeda35ebccefaff55ebf4184f9f5b081e0af59d6d054f3d46d",
        ),
    ] {
        let after = fs::read(dir.join(filename)).unwrap();
        verify(&after, hash);
        let (before, definition, data, roots) = if header == 5 {
            (&before5, 89, 94, [91, 92, 93])
        } else {
            (&before6, 106, 111, [108, 109, 110])
        };
        assert_eq!(after.len(), before.len());
        cases.push(Case {
            header,
            recipe,
            before: before.clone(),
            after,
            definition,
            data,
            roots,
        });
    }
    assert_eq!(cases.len(), 16);
    cases
}
