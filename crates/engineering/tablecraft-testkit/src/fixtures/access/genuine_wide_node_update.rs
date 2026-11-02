//! Independent stable-count NODE UPDATE source facts; no production codecs.
use std::{fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Recipe {
    Noop,
    Boundary,
    Shift,
    Swap,
    Joint,
}

#[derive(Clone, Copy)]
pub struct RowFact {
    pub reference: u32,
    pub offset: usize,
}

pub struct Facts {
    pub money: bool,
    pub definition: u32,
    pub roots: [u32; 3],
    pub children: [[u32; 2]; 2],
    pub records: &'static [RowFact],
}

pub struct Assignment {
    pub row: usize,
    pub expected: Value,
    pub replacement: Value,
}

pub struct Case {
    pub table: &'static str,
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub facts: &'static Facts,
}

fn value(units: i64, money: bool) -> Value {
    if money {
        Value::Currency(Currency::from_scaled(units))
    } else {
        Value::Int64(units)
    }
}

pub fn original_rows(case: &Case) -> Vec<Vec<Value>> {
    (1..=400)
        .map(|id| {
            vec![
                Value::Int32(id),
                value(if id == 1 { i64::MAX } else { 0 }, case.facts.money),
                Value::Int32(id * 17),
            ]
        })
        .collect()
}

pub fn groups(case: &Case) -> Vec<(&'static str, Vec<Assignment>)> {
    let updates = match case.recipe {
        Recipe::Noop => vec![(0, i64::MAX, i64::MAX)],
        Recipe::Boundary | Recipe::Joint => vec![(199, 0, 1)],
        Recipe::Shift => (1..400).map(|ordinal| (ordinal, 0, -1)).collect(),
        Recipe::Swap => vec![(0, i64::MAX, 0), (199, 0, i64::MAX)],
    };
    let updates = updates
        .into_iter()
        .map(|(row, expected, replacement)| Assignment {
            row,
            expected: value(expected, case.facts.money),
            replacement: value(replacement, case.facts.money),
        })
        .collect();
    let mut groups = vec![("KeyValue", updates)];
    if case.recipe == Recipe::Joint {
        groups.push((
            "Marker",
            vec![Assignment {
                row: 199,
                expected: Value::Int32(3400),
                replacement: Value::Int32(42),
            }],
        ));
    }
    groups
}

pub fn changed_rows(recipe: Recipe) -> usize {
    match recipe {
        Recipe::Noop => 0,
        Recipe::Boundary | Recipe::Joint => 1,
        Recipe::Shift => 399,
        Recipe::Swap => 2,
    }
}

pub fn cell(value: &Value) -> Value {
    match value {
        Value::Int32(value) => Value::Int32(*value),
        Value::Int64(value) => Value::Int64(*value),
        Value::Currency(value) => Value::Currency(*value),
        Value::Null => Value::Null,
        _ => panic!("outside frozen fixed carriers"),
    }
}

fn load(
    names: (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    ),
    recipe: Recipe,
    facts: &'static Facts,
    verify: &mut impl FnMut(&[u8], &str),
) -> Case {
    let (table, old_name, old_hash, new_name, new_hash) = names;
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_WIDE_NODE_UPDATE_SOURCE_DIR")
            .expect("qualified source directory"),
    );
    let before = fs::read(directory.join(old_name)).unwrap();
    let after = fs::read(directory.join(new_name)).unwrap();
    verify(&before, old_hash);
    verify(&after, new_hash);
    Case {
        table,
        recipe,
        before,
        after,
        facts,
    }
}

include!("genuine_wide_node_update/pins.rs");

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    all(&mut verify)
}
