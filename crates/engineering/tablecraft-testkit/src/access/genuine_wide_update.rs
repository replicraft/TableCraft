//! Independent original requests and literal whole-image wider UPDATE fixtures.
use std::{fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

pub struct Assignment {
    pub row: usize,
    pub expected: Value,
    pub replacement: Value,
}

pub struct Layout {
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
    pub record_length: usize,
}

pub struct Case {
    pub table: &'static str,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub wire: Vec<u8>,
    pub changed: usize,
    pub groups: Vec<(&'static str, Vec<Assignment>)>,
    pub layout: Layout,
}

type SourceCase = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    usize,
);

fn load(
    facts: SourceCase,
    groups: Vec<(&'static str, Vec<Assignment>)>,
    layout: Layout,
    verify: &mut impl FnMut(&[u8], &str),
) -> Case {
    let (table, recipe, old_name, old_hash, new_name, new_hash, hex, changed) = facts;
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_WIDE_UPDATE_SOURCE_DIR").expect("qualified source directory"),
    );
    let before = fs::read(directory.join(old_name)).unwrap();
    let after = fs::read(directory.join(new_name)).unwrap();
    verify(&before, old_hash);
    verify(&after, new_hash);
    let wire = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    Case {
        table,
        recipe,
        before,
        after,
        wire,
        changed,
        groups,
        layout,
    }
}

include!("genuine_wide_update/pins.rs");

/// Every original and literal AFTER pin is verified before returning any case.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    all(&mut verify)
}

pub fn cell(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Int32(value) => Value::Int32(*value),
        Value::Int64(value) => Value::Int64(*value),
        Value::Currency(value) => Value::Currency(*value),
        _ => panic!("outside independently frozen scalar carriers"),
    }
}

pub fn forged_offsets(case: &Case) -> Vec<usize> {
    let definition = case.layout.definition as usize * 4096;
    let data = case.layout.data as usize * 4096;
    let mut offsets = vec![
        0,
        4096 + 100,
        definition + 16,
        definition + 67,
        definition + 79,
        definition + 91,
        data + 2,
        data + 14,
        data + 4095,
        data + 4096 - 2 * case.layout.record_length,
    ];
    for root in case.layout.roots {
        let page = root as usize * 4096;
        offsets.extend([page + 2, page + 27, page + 480, page + 484, page + 4095]);
    }
    offsets
}
