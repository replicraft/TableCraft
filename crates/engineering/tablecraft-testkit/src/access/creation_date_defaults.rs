//! Pure source pins and value expectations; concrete creator requests belong to consumers.
use std::{fs, path::PathBuf};
use tablecraft_core::values::{LegacyDateTime, Value};

#[derive(Clone, Copy)]
pub struct Case {
    pub id: &'static str,
    pub expression: &'static str,
    pub bits: u64,
}

pub const CASES: [Case; 6] = [
    Case {
        id: "epoch-zero",
        expression: "#1899-12-30 00:00:00#",
        bits: 0x0000000000000000,
    },
    Case {
        id: "before-epoch-quarter",
        expression: "#1899-12-29 06:00:00#",
        bits: 0xbff4000000000000,
    },
    Case {
        id: "leap-noon",
        expression: "#2000-02-29 12:00:00#",
        bits: 0x40e1dd3000000000,
    },
    Case {
        id: "minimum",
        expression: "#0100-01-01 00:00:00#",
        bits: 0xc124103400000000,
    },
    Case {
        id: "maximum",
        expression: "#9999-12-31 00:00:00#",
        bits: 0x4146924080000000,
    },
    Case {
        id: "fractional-epoch",
        expression: "#1899-12-30 12:34:56.125#",
        bits: 0x3fe0c6be635dad52,
    },
];

pub const ALIAS_BITS: [u64; 4] = [
    0x8000000000000000,
    0xbfd0000000000000,
    0x0000000000000001,
    0x3fefffffffffffff,
];

pub fn date(bits: u64) -> Value {
    Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
}

pub fn original_row() -> Vec<Value> {
    vec![Value::Int32(1), date(0x3ff4000000000000)]
}

pub fn expected_rows(case: Case, phase: &str) -> Vec<Vec<Value>> {
    let mut rows = vec![original_row()];
    match phase {
        "before" => {}
        "omitted" => rows.push(vec![Value::Int32(2), date(case.bits)]),
        "supplied-null" => rows.push(vec![Value::Int32(3), Value::Null]),
        _ => panic!("closed frozen source phase"),
    }
    rows
}

pub struct Sources {
    directory: PathBuf,
}

impl Sources {
    pub fn load(mut verify: impl FnMut(&[u8], &str)) -> Self {
        let directory = PathBuf::from(
            std::env::var_os("TABLECRAFT_DATE_DEFAULT_SOURCE_DIR")
                .expect("explicit frozen source36 directory"),
        );
        assert!(directory.is_absolute());
        for (name, digest) in PINS {
            let raw = fs::read(directory.join(name)).unwrap();
            assert_eq!(raw.len(), 36864);
            verify(&raw, digest);
        }
        Self { directory }
    }

    pub fn bytes(&self, header: u8, case: Case, phase: &str) -> Vec<u8> {
        fs::read(
            self.directory
                .join(format!("h{header}-{}-{phase}.accdb", case.id)),
        )
        .unwrap()
    }
}

const PINS: &[(&str, &str)] = &[
    (
        "h5-epoch-zero-before.accdb",
        "17e5d7914005b28d36a0ac097d7ebf0cfc4c91057f87129bd6714d924ebd98db",
    ),
    (
        "h5-epoch-zero-omitted.accdb",
        "ccf2d5a5971a39bc736f41610f1d1f9a9ca8c38d49202d4a5b4b6203d94807a3",
    ),
    (
        "h5-epoch-zero-supplied-null.accdb",
        "3b4c3fa4f8711f6b232f56f5ee8ada4d274983845e366c36cbf5372e21e35a2c",
    ),
    (
        "h5-before-epoch-quarter-before.accdb",
        "83fb8654761d29b3ab8c56d057f00ab4e91923e84fa4291ab046ac5b3880d0f3",
    ),
    (
        "h5-before-epoch-quarter-omitted.accdb",
        "af746e6949128c7af92cc8bda824a46dfa393aeaf149b4635dd0f10b5f487479",
    ),
    (
        "h5-before-epoch-quarter-supplied-null.accdb",
        "7cf37e8df7869e2338d07d3902bf57ac04bab2c4dae5051b7b67737f39f6d32b",
    ),
    (
        "h5-leap-noon-before.accdb",
        "512d101456e868150dc07c6a5b490030857d8eccf8a0179993df3ee2ada99990",
    ),
    (
        "h5-leap-noon-omitted.accdb",
        "7ae3f48b3fb00d4665da128445be547940a39b16c43cf5879f1ca60e85699dbc",
    ),
    (
        "h5-leap-noon-supplied-null.accdb",
        "f3f1bbb3fbee4c1cc02057b989d2a18fa5eb2624080db5febde9049ff951c861",
    ),
    (
        "h5-minimum-before.accdb",
        "9a9672b0096721af519d310739720152a8cc51efb8ffdd293d86f1cf60f4d6d6",
    ),
    (
        "h5-minimum-omitted.accdb",
        "66c8c1a697a4c8d5c83ba8bc9b2df73964355f12973fdf1fcd49999107a997bf",
    ),
    (
        "h5-minimum-supplied-null.accdb",
        "a014abfd6f36ea1df6d5f232710f56a629bfc4dc911775dc94f861c72999e37c",
    ),
    (
        "h5-maximum-before.accdb",
        "b693d3a687df7690754ca7aaeb961494f12bafd9e796f62296571c2be1cb1723",
    ),
    (
        "h5-maximum-omitted.accdb",
        "b73a08fad29a7472d64ddcf129f5ff297b0381f41171ea3870349a26eb871350",
    ),
    (
        "h5-maximum-supplied-null.accdb",
        "3a0bee7a7e296c6bff8d6abee7c128e4d0499f29d44215a6ebc0f65e2b08246f",
    ),
    (
        "h5-fractional-epoch-before.accdb",
        "4d366719d2985d4419d28af5edd0e48d058dd53a8553c84a03c9e9ce8c434c1b",
    ),
    (
        "h5-fractional-epoch-omitted.accdb",
        "e5f8a10c5e85f80d5c26febeabd1ff023845990ceeb1a916e93eae4208e10ae0",
    ),
    (
        "h5-fractional-epoch-supplied-null.accdb",
        "4a744348c4e76a1ae3f9b889e39f0c60022e7703d3b90592b2cb4a61e4b040c1",
    ),
    (
        "h6-epoch-zero-before.accdb",
        "e199b1998d71fb3581bc09a803ff9ddf59fbd3192d8b8b4c1e9cd7d61cba1231",
    ),
    (
        "h6-epoch-zero-omitted.accdb",
        "b831fecf79c7d150449a7c8931f1d81fcccb64ad93290712ddc5435725cfffec",
    ),
    (
        "h6-epoch-zero-supplied-null.accdb",
        "175e524f98d505c057f049a7f27f72d21f05d7a9d3fc20196176e4055c556dab",
    ),
    (
        "h6-before-epoch-quarter-before.accdb",
        "5b42590d20f245a2dbbbb475e7199ff799d6222a000c181a91e2a09c59d1b333",
    ),
    (
        "h6-before-epoch-quarter-omitted.accdb",
        "79dfb48d8a980423a508e5c23ef13e671a058f0669ceeb7c3053979454f129a5",
    ),
    (
        "h6-before-epoch-quarter-supplied-null.accdb",
        "ad1d920789242927366c6d13ff42b97892408325febdd02fd7f94292c00b4dca",
    ),
    (
        "h6-leap-noon-before.accdb",
        "8538414783c7b0c3b74bba8a785f4bfbbb957016266a39cf8f4674dfcdf594b4",
    ),
    (
        "h6-leap-noon-omitted.accdb",
        "e9d169fd91b1ab2ff5b58558db938db403dee2a0ebe80dadccf7bc0109f3437a",
    ),
    (
        "h6-leap-noon-supplied-null.accdb",
        "dae3c9b8703c428f7269a3a42b673c431df388f563eb935bea922b9b03b7f2eb",
    ),
    (
        "h6-minimum-before.accdb",
        "17c3a924747e11d732c5ee8be9d3908c25d7f068b8bb1aa8692c21ff66d60668",
    ),
    (
        "h6-minimum-omitted.accdb",
        "a12d725299a4e55dd5a08f92117851795cd5819c8ab11c881c6c02232bc2b171",
    ),
    (
        "h6-minimum-supplied-null.accdb",
        "ddbc26110882face1bbb1a479a929de3ae18b2900f8ac2d3480352a408c28a7a",
    ),
    (
        "h6-maximum-before.accdb",
        "1b911de04769d8ebd9c5f1ce6468d515428859bcef1532feef8bfa10245d4cd3",
    ),
    (
        "h6-maximum-omitted.accdb",
        "8c76538e03de4c98ac7f71e549ac4bc06ce4088f977866e9ef1b1a0a7faf0d3f",
    ),
    (
        "h6-maximum-supplied-null.accdb",
        "fa19ac0f7a02f0092c5b31046ed80c370c7e207c936c712b3a492c61a7f5733d",
    ),
    (
        "h6-fractional-epoch-before.accdb",
        "fae5f6d72c369e55f02444bcae3c8a08d3c3735e0954c68b8832c55eeaec7551",
    ),
    (
        "h6-fractional-epoch-omitted.accdb",
        "adc1f460cc7bcbb3c3ecebc906fa4902730f738a70838ca907e722f6e14de6d0",
    ),
    (
        "h6-fractional-epoch-supplied-null.accdb",
        "f38c538a71654043d2f23debedeea71ebd17aee3aa1c5925229a1a1ce9311041",
    ),
];
