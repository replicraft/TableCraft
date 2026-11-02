//! Original-only default-presence images; no engine or producer dependency.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

#[derive(Clone, Copy, Debug)]
pub enum Recipe {
    OmittedConstants,
    ExplicitNull,
    AllOverrides,
}

pub const RECIPES: [Recipe; 3] = [
    Recipe::OmittedConstants,
    Recipe::ExplicitNull,
    Recipe::AllOverrides,
];

impl Recipe {
    pub fn id(self) -> &'static str {
        match self {
            Self::OmittedConstants => "omitted-constants",
            Self::ExplicitNull => "explicit-null",
            Self::AllOverrides => "all-overrides",
        }
    }

    /// None is Omitted; Some(Value::Null) is explicitly supplied stored NULL.
    pub fn values(self) -> Vec<Option<Value>> {
        match self {
            Self::OmittedConstants => vec![Some(Value::Int32(24)), None, None, None, None, None],
            Self::ExplicitNull => vec![
                Some(Value::Int32(25)),
                Some(Value::Null),
                Some(Value::Null),
                Some(Value::Boolean(false)),
                Some(Value::Null),
                Some(Value::Null),
            ],
            Self::AllOverrides => self.final_values().into_iter().map(Some).collect(),
        }
    }

    pub fn final_values(self) -> Vec<Value> {
        match self {
            Self::OmittedConstants => vec![
                Value::Int32(24),
                Value::Int32(7),
                Value::Text("seed".into()),
                Value::Boolean(true),
                Value::Currency(Currency::from_scaled(-10001)),
                Value::Null,
            ],
            Self::ExplicitNull => vec![
                Value::Int32(25),
                Value::Null,
                Value::Null,
                Value::Boolean(false),
                Value::Null,
                Value::Null,
            ],
            Self::AllOverrides => vec![
                Value::Int32(26),
                Value::Int32(-9),
                Value::Text("override".into()),
                Value::Boolean(false),
                Value::Currency(Currency::from_scaled(12345)),
                Value::Int32(99),
            ],
        }
    }

    pub fn sql(self) -> &'static str {
        match self {
            Self::OmittedConstants => "INSERT INTO Defaults ([Id]) VALUES (24)",
            Self::ExplicitNull => {
                "INSERT INTO Defaults ([Id], [Score], [Caption], [Flag], [Money], [Optional]) VALUES (25, NULL, NULL, False, NULL, NULL)"
            }
            Self::AllOverrides => {
                "INSERT INTO Defaults ([Id], [Score], [Caption], [Flag], [Money], [Optional]) VALUES (26, -9, 'override', False, CCur(1.2345), 99)"
            }
        }
    }
}

pub struct Case {
    pub header: u8,
    pub definition: u32,
    pub data: u32,
    pub index: u32,
    pub before: Vec<u8>,
    pub after: Vec<Vec<u8>>,
}

impl Case {
    pub fn expected(&self, recipe: Recipe) -> &[u8] {
        let position = match recipe {
            Recipe::OmittedConstants => 0,
            Recipe::ExplicitNull => 1,
            Recipe::AllOverrides => 2,
        };
        &self.after[position]
    }
}

pub fn original_values() -> Vec<Value> {
    vec![
        Value::Int32(1),
        Value::Int32(-1),
        Value::Text("original".into()),
        Value::Boolean(false),
        Value::Currency(Currency::from_scaled(25000)),
        Value::Null,
    ]
}

/// Verify all eight pinned images before any caller starts its first plan.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_NORMAL_DEFAULT_AFTER_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, definition, data, index, length, pins) in [
        (
            5,
            89,
            92,
            91,
            446464,
            [
                "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
                "33c0ee28d7fcfb7cf9379efb9c89b2e2f261f00f17eba12f78eb179841a3c850",
                "99fae1edfabe1f593fce28db48a28ab7818679fb6f9cdcbbfe586c8954ebf153",
                "d1e45700af63d9ca71e84bcd7346028ddfd1b50c1a1c0f7ace321ab2cd9454da",
            ],
        ),
        (
            6,
            106,
            109,
            108,
            520192,
            [
                "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
                "4d46f78ba407c6b62314656d2688f9652de2afb4cb4eb1461b5a759991352bdd",
                "7af984c59831f415f42626432bbe9762136a55373431fb423c9d5c467d346ac1",
                "1c42c8d5e8bf2c672822a354b12120f57a319e8ee9722535eacedf22076220be",
            ],
        ),
    ] {
        let before = fs::read(directory.join(format!("h{header}-default-original.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        verify(&before, pins[0]);
        let mut after = Vec::new();
        for (recipe, pin) in RECIPES.into_iter().zip(&pins[1..]) {
            let bytes =
                fs::read(directory.join(format!("h{header}-default-{}.accdb", recipe.id())))
                    .unwrap();
            assert_eq!(bytes.len(), length);
            verify(&bytes, pin);
            after.push(bytes);
        }
        cases.push(Case {
            header,
            definition,
            data,
            index,
            before,
            after,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
