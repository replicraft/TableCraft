//! Exact retained originals and independent literal Marker outcomes only.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

pub struct Case {
    pub header: u8,
    pub table: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub rows: usize,
    pub key: Value,
    pub data: u32,
}

fn after_digest(header: u8, table: &str) -> &'static str {
    match (header, table) {
        (5, "ByteKeys") => "5bd24a2bc41437e77c1ca5ca8d4a566fea4734ea17d38ae8f339fa89454e735e",
        (5, "Int16Keys") => "e40b62532280419c514a6ecb16a13482e019e8073813b9fbdd4821f0364303ff",
        (5, "Int64Keys") => "4f1e73dc79f530c53fe6ba63bc4bedd7c509cad6ff280ca994a92b16201fa6d7",
        (5, "CurrencyKeys") => "5d1aa7b502e1c032acc9939602d6cd7aa7d00f0a4324b1bc12529c9cef6343fe",
        (6, "ByteKeys") => "c3b070704a6583062721ab82de55c023db204391c07da9b243c63b6e4e2c56be",
        (6, "Int16Keys") => "8edf4bf01f40a1638975d12aeda06b1c02a7eff74a74461312538470d6ee3dd3",
        (6, "Int64Keys") => "7913ac0871b4f55b9da5aecbcad6d5139a2b4f6fbefd0124bdfdaea2aa616875",
        (6, "CurrencyKeys") => "c49f4396284893a8f76c257bf123981bdbda690b89b29fbedf84ddd8c3e515d9",
        _ => panic!("only eight frozen Marker outcomes"),
    }
}

/// All original/AFTER pins are checked before any case is returned to a caller.
/// The caller supplies its existing SHA implementation; no engine is imported.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let originals =
        PathBuf::from(env::var_os("TABLECRAFT_RETAINED_NUMERIC_ORIGINALS_DIR").unwrap());
    let expected = PathBuf::from(env::var_os("TABLECRAFT_RETAINED_NUMERIC_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    let mut cases = Vec::new();
    for (profile, header, length, digest, data) in [
        (
            "V2016",
            5_u8,
            757760,
            "8e976662f1c2ddf2e7616870082a71da3a618017ab0cca6faf78f7fe301350a6",
            [94, 114, 133, 149],
        ),
        (
            "V2019",
            6_u8,
            827392,
            "dcb7f14d9eab21dd9f518ddd3aafdc90b06f9325a20b5bbe60cdc44303f39dd7",
            [111, 131, 150, 166],
        ),
    ] {
        let before = fs::read(originals.join(format!("{profile}-before.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        verify(&before, digest);
        assert_eq!(&before[20..24], &u32::from(header).to_le_bytes());
        for ((table, rows, key), data) in [
            ("ByteKeys", 900, Value::Byte(0)),
            ("Int16Keys", 900, Value::Int16(0)),
            ("Int64Keys", 400, Value::Int64(0)),
            (
                "CurrencyKeys",
                400,
                Value::Currency(Currency::from_scaled(0)),
            ),
        ]
        .into_iter()
        .zip(data)
        {
            let after = fs::read(expected.join(format!("{profile}-{table}-marker.accdb"))).unwrap();
            assert_eq!(after.len(), length);
            verify(&after, after_digest(header, table));
            cases.push(Case {
                header,
                table,
                before: before.clone(),
                after,
                rows,
                key,
                data,
            });
        }
    }
    assert_eq!(cases.len(), 8);
    cases
}
