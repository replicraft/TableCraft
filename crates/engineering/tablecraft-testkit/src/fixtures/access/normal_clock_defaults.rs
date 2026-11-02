//! Pure fixture bytes, original presence and exact civil/DATE laws; no engine or clock.
use std::{collections::BTreeMap, env, fs, path::PathBuf};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Value};

const PINS: [(&str, &str); 18] = [
    (
        "h5-date-before-epoch-six.accdb",
        "fbdf8b6b7f87d47803ad973dd6f131ffdd3576d920870a2e8dd72d0e8e33ec6d",
    ),
    (
        "h5-date-before.accdb",
        "d02cd33da769ad948af8aee9a4d568ae03f47e521149b484b6dba2bf9fb27575",
    ),
    (
        "h5-date-leap-noon.accdb",
        "0d7389a29ee81529bdfa9e15201340d0e90c7b859348a69bc23a37ab833cc144",
    ),
    (
        "h5-date-supplied-null.accdb",
        "083e633e696d15a1e39833aa215c3a516f4365b727365c812451ea3b9aaad25d",
    ),
    (
        "h5-now-before-epoch-six.accdb",
        "5b03e1a88c03c3733ce3b6865314db63771c646fd872f93b7a77cfa669ac35a3",
    ),
    (
        "h5-now-before.accdb",
        "0cf0c1a260f17d9bca9a4e42d513871f23bfa90d760a3a30f77dee767f9b3624",
    ),
    (
        "h5-now-leap-batch.accdb",
        "0ed4c1883176763b9fdafe981aad87bff2076079538edd07e1691651cad18dbf",
    ),
    (
        "h5-now-leap-noon.accdb",
        "bc06dc447e11ae9a0da3e2958c919d7d94e556cf8dda9bab2fdc9a987f6bd9ec",
    ),
    (
        "h5-now-supplied-null.accdb",
        "1a5668d6bd4b81f22c013595b04353550fe8b54470e8f081f4147dbcf53d5b07",
    ),
    (
        "h6-date-before-epoch-six.accdb",
        "ed010e1e7958950195160bc4ea0fd21aabb6322b04ac8832a46fded94b3f8303",
    ),
    (
        "h6-date-before.accdb",
        "1ee5de60f2670d0a99f0a51f6cbf19b4449056794666bf57ef198895ebf0aacb",
    ),
    (
        "h6-date-leap-noon.accdb",
        "7148462aee7e20655889c15aa6a83295c7b2b0ec9f805adabd140007c1d450ab",
    ),
    (
        "h6-date-supplied-null.accdb",
        "efcb9837b8c72bc9c90b7c360a742f3d4121f5e4d987489a3600a8e20f4dd374",
    ),
    (
        "h6-now-before-epoch-six.accdb",
        "db8b9fcfd89841c14706a90b66429fb66e141fe27e292c41ec0f6abc2caaf3cd",
    ),
    (
        "h6-now-before.accdb",
        "f696bcfc0637365a06969d3a6cd4cdb8f9e1bdb5626f7378dbef0fd92b6f09c2",
    ),
    (
        "h6-now-leap-batch.accdb",
        "a985855a7d854a4f37e1fe62ec00341fbc7be884dea1cdca8e9bc80aef109be4",
    ),
    (
        "h6-now-leap-noon.accdb",
        "f846fcf352ddea10415d801c97855399d5cafa6172c73ba99052f718eb177f1f",
    ),
    (
        "h6-now-supplied-null.accdb",
        "c60fdaf93331002a90ecf14a8734c2cd57c70b5edaf413caddc9d221f04c7f44",
    ),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Function {
    Date,
    Now,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Recipe {
    LeapNoon,
    BeforeEpochSix,
    SuppliedNull,
    LeapBatch,
    Empty,
}

pub enum Field {
    Supplied(Value),
    Omitted,
}

pub struct Sources {
    images: BTreeMap<String, Vec<u8>>,
}

pub struct Case<'a> {
    pub header: u32,
    pub function: Function,
    pub recipe: Recipe,
    pub before: &'a [u8],
    pub after: &'a [u8],
}

impl Sources {
    /// Verify every immutable image before returning any operation context.
    pub fn load(mut verify: impl FnMut(&[u8], &str)) -> Self {
        let directory = PathBuf::from(
            env::var_os("TABLECRAFT_CLOCK_DEFAULT_SOURCE_DIR")
                .expect("explicit source18 directory"),
        );
        assert!(directory.is_absolute());
        let mut images = BTreeMap::new();
        for (name, wanted) in PINS {
            let bytes = fs::read(directory.join(name)).unwrap();
            assert_eq!(bytes.len(), 9 * 4096);
            verify(&bytes, wanted);
            assert!(images.insert(name.to_owned(), bytes).is_none());
        }
        Self { images }
    }

    pub fn cases(&self) -> Vec<Case<'_>> {
        let mut cases = Vec::new();
        for header in [5_u32, 6] {
            for function in [Function::Date, Function::Now] {
                let label = if function == Function::Date {
                    "date"
                } else {
                    "now"
                };
                let before = self.images[&format!("h{header}-{label}-before.accdb")].as_slice();
                let mut recipes = vec![
                    Recipe::LeapNoon,
                    Recipe::BeforeEpochSix,
                    Recipe::SuppliedNull,
                    Recipe::Empty,
                ];
                if function == Function::Now {
                    recipes.push(Recipe::LeapBatch);
                }
                for recipe in recipes {
                    let phase = match recipe {
                        Recipe::LeapNoon => "leap-noon",
                        Recipe::BeforeEpochSix => "before-epoch-six",
                        Recipe::SuppliedNull => "supplied-null",
                        Recipe::LeapBatch => "leap-batch",
                        Recipe::Empty => "before",
                    };
                    let after = self.images[&format!("h{header}-{label}-{phase}.accdb")].as_slice();
                    cases.push(Case {
                        header,
                        function,
                        recipe,
                        before,
                        after,
                    });
                }
            }
        }
        assert_eq!(cases.len(), 18);
        cases
    }
}

impl Case<'_> {
    pub fn needs_clock(&self) -> bool {
        matches!(
            self.recipe,
            Recipe::LeapNoon | Recipe::BeforeEpochSix | Recipe::LeapBatch
        )
    }

    /// Original host civil input includes fractions, which Date/Now truncate.
    pub fn clock_input(&self) -> ExtendedDateTime {
        if self.recipe == Recipe::BeforeEpochSix {
            ExtendedDateTime::new(1899, 12, 29, 6, 0, 0, 0).unwrap()
        } else {
            ExtendedDateTime::new(2000, 2, 29, 12, 0, 0, 7_654_321).unwrap()
        }
    }

    pub fn clock_text(&self) -> &'static str {
        if self.recipe == Recipe::BeforeEpochSix {
            "1899-12-29T06:00:00"
        } else {
            "2000-02-29T12:00:00.7654321"
        }
    }

    pub fn requests(&self) -> Vec<Vec<Field>> {
        match self.recipe {
            Recipe::Empty => Vec::new(),
            Recipe::SuppliedNull => vec![vec![
                Field::Supplied(Value::Int32(3)),
                Field::Supplied(Value::Null),
            ]],
            Recipe::LeapBatch => vec![
                vec![Field::Supplied(Value::Int32(2)), Field::Omitted],
                vec![Field::Supplied(Value::Int32(4)), Field::Omitted],
            ],
            _ => vec![vec![Field::Supplied(Value::Int32(2)), Field::Omitted]],
        }
    }

    pub fn sql(&self) -> Option<&'static str> {
        match self.recipe {
            Recipe::Empty | Recipe::LeapBatch => None,
            Recipe::SuppliedNull => Some("INSERT INTO Items ([Id], [When]) VALUES (3, NULL)"),
            _ => Some("INSERT INTO Items ([Id]) VALUES (2)"),
        }
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        let mut rows = vec![vec![
            Value::Int32(1),
            Value::DateTime(LegacyDateTime::from_bits(0x3ff4000000000000).unwrap()),
        ]];
        if self.recipe == Recipe::Empty {
            return rows;
        }
        if self.recipe == Recipe::SuppliedNull {
            rows.push(vec![Value::Int32(3), Value::Null]);
            return rows;
        }
        let bits = match (self.function, self.recipe) {
            (Function::Date, Recipe::BeforeEpochSix) => 0xbff0000000000000,
            (Function::Now, Recipe::BeforeEpochSix) => 0xbff4000000000000,
            (Function::Date, _) => 0x40e1dd2000000000,
            (Function::Now, _) => 0x40e1dd3000000000,
        };
        let value = Value::DateTime(LegacyDateTime::from_bits(bits).unwrap());
        rows.push(vec![Value::Int32(2), value.clone()]);
        if self.recipe == Recipe::LeapBatch {
            rows.push(vec![Value::Int32(4), value]);
        }
        rows
    }

    /// Independent literal TCNA5 framing over the exact original source request.
    /// The caller supplies the separately computed original-image digest.
    pub fn intent_wire(&self, binding: [u8; 32]) -> Vec<u8> {
        let rows = self.requests();
        let nested_length = 73 + 28 * rows.len();
        let mut original = Vec::from(*b"TCNA0002");
        original.extend_from_slice(&(nested_length as u32).to_le_bytes());
        original.extend_from_slice(&[2, 0, 0, 0]);
        original.extend_from_slice(&(self.before.len() as u64).to_le_bytes());
        original.extend_from_slice(&binding);
        for word in [5_u32, rows.len() as u32, 0] {
            original.extend_from_slice(&word.to_le_bytes());
        }
        original.extend_from_slice(b"Items");
        for row in rows {
            original.extend_from_slice(&28_u32.to_le_bytes());
            original.extend_from_slice(&2_u32.to_le_bytes());
            for field in row {
                match field {
                    Field::Omitted => original.extend_from_slice(&[8, 0, 0, 0, 0, 0, 0, 0]),
                    Field::Supplied(Value::Int32(value)) => {
                        original.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0]);
                        original.extend_from_slice(&value.to_le_bytes());
                    }
                    Field::Supplied(Value::Null) => original.extend_from_slice(&[0; 8]),
                    _ => panic!("closed original source carrier"),
                }
            }
        }
        assert_eq!(original.len(), nested_length);
        let mut raw = Vec::from(*b"TCNA0005");
        raw.extend_from_slice(&((32 + nested_length) as u32).to_le_bytes());
        raw.extend_from_slice(&[5, 0, 0, 0]);
        let mut civil = [0_u8; 16];
        if self.needs_clock() {
            civil[0] = 1;
            let input = self.clock_input();
            let (year, month, day) = input.date();
            let (hour, minute, second, _) = input.time();
            civil[2..4].copy_from_slice(&year.to_le_bytes());
            civil[4..9].copy_from_slice(&[month, day, hour, minute, second]);
        }
        raw.extend_from_slice(&civil);
        raw.extend_from_slice(&original);
        raw
    }
}
