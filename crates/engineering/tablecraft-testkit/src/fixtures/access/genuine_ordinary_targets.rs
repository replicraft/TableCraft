//! Frozen request/proof metadata and explicit private artifact loading only.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    ScoreSeventeen,
    ScoreClear,
    ScoreNoop,
    ScoreEmpty,
    ManualIdSeven,
    DuplicateId,
    StaleScore,
}

pub const POSITIVES: [Recipe; 5] = [
    Recipe::ScoreSeventeen,
    Recipe::ScoreClear,
    Recipe::ScoreNoop,
    Recipe::ScoreEmpty,
    Recipe::ManualIdSeven,
];

impl Recipe {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ScoreSeventeen => "score-seventeen",
            Self::ScoreClear => "score-clear",
            Self::ScoreNoop => "score-noop",
            Self::ScoreEmpty => "score-empty",
            Self::ManualIdSeven => "manual-id-seven",
            Self::DuplicateId => "id-duplicate",
            Self::StaleScore => "stale-score",
        }
    }
}

pub struct Case {
    pub before: Vec<u8>,
    pub after: Option<Vec<u8>>,
    pub before_digest: &'static str,
    pub after_digest: Option<&'static str>,
    pub column: &'static str,
    pub expected: Option<i32>,
    pub replacement: Option<i32>,
    pub empty: bool,
    pub changed_rows: usize,
    pub rows: Vec<Vec<Value>>,
    pub final_rows: Vec<Vec<Value>>,
}

pub fn value(value: Option<i32>) -> Value {
    value.map_or(Value::Null, Value::Int32)
}

fn original_digest(header: u8) -> &'static str {
    match header {
        5 => "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
        6 => "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
        _ => panic!("only the two frozen genuine headers are source inputs"),
    }
}

fn after_digest(header: u8, recipe: Recipe) -> Option<&'static str> {
    match (header, recipe) {
        (5, Recipe::ScoreSeventeen) => {
            Some("b4786e862d6eedde65eb6c557bd715f4fc738e957c43fec6cd282adbee598394")
        }
        (5, Recipe::ScoreClear) => {
            Some("6edc8a68775576262aabcaf3d60a07b43fef27098ad3391847cb52f91ca86512")
        }
        (6, Recipe::ScoreSeventeen) => {
            Some("56905f213eddf8f449deba3eace5304094dbe99e6c6b206229c3e846487a20fe")
        }
        (6, Recipe::ScoreClear) => {
            Some("bee8eacf6a06e196fc00ebf066a7d197e76b55884cc03e790c95e4082caf034e")
        }
        (5, Recipe::ManualIdSeven) => {
            Some("48e6d3cf92b98491c2ed3c752705e7542cc12e18156f81986f4da732666a171c")
        }
        (6, Recipe::ManualIdSeven) => {
            Some("4d3d527dcfa127d6e9084f4d8a7e11afd18db8c66933189b810dca5264feee8a")
        }
        (5 | 6, Recipe::ScoreNoop | Recipe::ScoreEmpty) => Some(original_digest(header)),
        (5 | 6, Recipe::DuplicateId | Recipe::StaleScore) => None,
        _ => panic!("unknown frozen source request"),
    }
}

/// Load outputs of the maintained pure create_expected tool, never a producer.
/// The caller/controller also freezes the whole directory before acceptance.
pub fn source(header: u8, recipe: Recipe) -> Case {
    let directory = PathBuf::from(
        env::var_os("TABLECRAFT_GENUINE_ORDINARY_EXPECTATIONS_DIR")
            .expect("set the explicit source-qualified artifact directory"),
    );
    assert!(directory.is_absolute());
    let before = fs::read(directory.join(format!("h{header}-original.accdb"))).unwrap();
    assert_eq!(before.len(), if header == 5 { 405504 } else { 475136 });
    assert_eq!(&before[20..24], &u32::from(header).to_le_bytes());
    let digest = after_digest(header, recipe);
    let after = digest
        .map(|_| fs::read(directory.join(format!("h{header}-{}.accdb", recipe.id()))).unwrap());
    if let Some(after) = &after {
        assert_eq!(after.len(), before.len());
    }
    let rows = vec![
        vec![Value::Int32(1), Value::Int32(-1), Value::Int32(0)],
        vec![Value::Int32(2), Value::Int32(0), Value::Int32(1)],
        vec![Value::Int32(3), Value::Int32(1), Value::Int32(2)],
    ];
    let (column, expected, replacement, empty, changed_rows) = match recipe {
        Recipe::ScoreSeventeen => ("Score", Some(-1), Some(17), false, 1),
        Recipe::ScoreClear => ("Score", Some(-1), None, false, 1),
        Recipe::ScoreNoop => ("Score", Some(-1), Some(-1), false, 0),
        Recipe::ScoreEmpty => ("Score", None, None, true, 0),
        Recipe::ManualIdSeven => ("Id", Some(1), Some(7), false, 1),
        Recipe::DuplicateId => ("Id", Some(1), Some(2), false, 0),
        Recipe::StaleScore => ("Score", Some(0), Some(17), false, 0),
    };
    let data = if header == 5 { 93 } else { 110 };
    for (slot, values) in [[1_i32, -1, 0], [2, 0, 1], [3, 1, 2]].iter().enumerate() {
        let start = data * 4096 + 4081 - slot * 15;
        assert_eq!(&before[start..start + 2], &[3, 0]);
        for (column, value) in values.iter().enumerate() {
            assert_eq!(
                &before[start + 2 + column * 4..start + 6 + column * 4],
                &value.to_le_bytes()
            );
        }
        assert_eq!(before[start + 14], 7);
    }
    let mut final_rows = rows.clone();
    if after.is_some() && !empty {
        final_rows[0][usize::from(column == "Score")] = value(replacement);
    }
    Case {
        before,
        after,
        before_digest: original_digest(header),
        after_digest: digest,
        column,
        expected,
        replacement,
        empty,
        changed_rows,
        rows,
        final_rows,
    }
}
