//! Pure source pins and literal rows; no engine, mode detection or generation.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

const PINS: [(&str, &str); 14] = [
    (
        "h5-increment-high-before.accdb",
        "2b980597bf79b28bc1b2685adccd3be53b00b3766b7e7c1718f264026bdb36c3",
    ),
    (
        "h6-increment-high-before.accdb",
        "bf61d8b25b04a34eee2d80565cb000b3551a258e0bb9e57569a7d52ad63f8dee",
    ),
    (
        "h5-increment-high-inserted.accdb",
        "561012288ff5a65b140704cf99475aa7edf5245135eb846b72430bbe5aed9ae4",
    ),
    (
        "h6-increment-high-inserted.accdb",
        "8af9b6e9bca59a68b25cd58170f0acca4396d0cb330f9af1aa04ab544b3ddc05",
    ),
    (
        "h5-increment-zero-before.accdb",
        "01fb598cf7bbe27bd17e7d7f4ad5f3c564c4a99dd59b2201dc7bbd44855292ac",
    ),
    (
        "h6-increment-zero-before.accdb",
        "420f7175bed4da81abf8da9d6d5c88497e479e052e488b60f63cf9b39da6a5d8",
    ),
    (
        "h5-increment-zero-inserted.accdb",
        "5f996ff001078d56e63e2bdf6bb26b2b002765f24a523ce2f7c3ce541b56f9c0",
    ),
    (
        "h6-increment-zero-inserted.accdb",
        "6ec8818ee3b58ecbef37c2b18fda61d02fdf4a349d520757fb0a7ee9547063d1",
    ),
    (
        "h5-random-special-default.accdb",
        "d45a6157d94c892b8963449796b30d5e54512de1894fa9def47c042b240c2638",
    ),
    (
        "h6-random-special-default.accdb",
        "8d729549fed474fdd3a7bf3731254ec46f100275e480993614ecf78e531dad2e",
    ),
    (
        "h5-unknown-generator-default.accdb",
        "a52b08c097b3fd1306a002cabb849dc804962a50a0a000de63f76603e87a1f01",
    ),
    (
        "h6-unknown-generator-default.accdb",
        "8bed5e8b8fe8587cc193e8a4a79029ba766df2b656950036407910698784342c",
    ),
    (
        "h5-increment-maximum.accdb",
        "b41015c0d7ef23cede0edcfbbc64d6c738c694096560eeb5041ef2914cd7f679",
    ),
    (
        "h6-increment-maximum.accdb",
        "5c0135b57d99f70c41d418e06feefddaa976e6b7af07585cb14793fdf0c78ab5",
    ),
];

#[derive(Clone, Copy)]
pub enum Recipe {
    High,
    Zero,
}

pub const RECIPES: [Recipe; 2] = [Recipe::High, Recipe::Zero];

impl Recipe {
    pub fn before(self) -> &'static str {
        match self {
            Self::High => "increment-high-before",
            Self::Zero => "increment-zero-before",
        }
    }
    pub fn after(self) -> &'static str {
        match self {
            Self::High => "increment-high-inserted",
            Self::Zero => "increment-zero-inserted",
        }
    }
    pub fn generated(self) -> i32 {
        match self {
            Self::High => 138,
            Self::Zero => 1,
        }
    }
    pub fn rows(self, inserted: bool) -> Vec<Vec<Value>> {
        let mut rows = match self {
            Self::High => vec![vec![Value::Int32(1), Value::Int32(10)]],
            Self::Zero => Vec::new(),
        };
        if inserted {
            rows.push(vec![Value::Int32(self.generated()), Value::Int32(17)]);
        }
        rows
    }
}

pub struct Sources {
    files: Vec<(String, Vec<u8>)>,
}

impl Sources {
    pub fn load(mut verify: impl FnMut(&[u8], &str)) -> Self {
        let root = PathBuf::from(env::var_os("TABLECRAFT_NORMAL_AUTONUMBER_SOURCE_DIR").unwrap());
        assert!(root.is_absolute());
        let mut files = Vec::new();
        for (name, wanted) in PINS {
            let raw = fs::read(root.join(name)).unwrap();
            assert_eq!(raw.len(), 40960);
            verify(&raw, wanted);
            files.push((name.to_owned(), raw));
        }
        Self { files }
    }
    pub fn bytes(&self, header: u32, recipe: &str) -> &[u8] {
        let name = format!("h{header}-{recipe}.accdb");
        &self.files.iter().find(|(key, _)| key == &name).unwrap().1
    }
}
