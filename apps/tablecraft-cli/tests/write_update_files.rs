//! Source-first file UPDATE images; no transport or native writer calls.
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16_long_value_updates_contract/routing_cases.rs"]
mod cases;
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
pub mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16_long_value_updates_contract/forwarding_case.rs"]
mod forwarding;
#[path = "write_update_files/model.rs"]
mod large;
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16_long_value_updates_contract/model.rs"]
pub mod original;

use std::io::Cursor;
use tablecraft_ace::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate, check_table,
    check_table_constraints, check_table_long_values, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

#[test]
fn qualify_file_update_sources_before_transport() {
    let mut selected = cases::cases();
    let position = selected
        .iter()
        .position(|case| case.column == "Ole" && case.value == Value::Binary(vec![8]))
        .unwrap();
    selected[position] = forwarding::case();
    selected.push(large::case());
    for case in selected {
        for (bytes, value) in [
            (&case.before, &case.before_value),
            (&case.expected, &case.value),
        ] {
            let rows = original::read(bytes);
            let column = if case.column == "Memo" { 1 } else { 2 };
            assert!(rows.len() == 1 && rows[0][column] == *value);
            assert_eq!(rows[0][0], Value::Int32(1));
            assert_eq!(
                rows[0][if column == 1 { 2 } else { 1 }],
                if column == 1 {
                    Value::Binary(vec![7])
                } else {
                    Value::Text("A".into())
                }
            );
            original::check(bytes);
        }
    }
    qualify_large_chain_and_protected_prefix();
}

fn qualify_large_chain_and_protected_prefix() {
    let case = large::case();
    assert_eq!(case.before.len(), 24 * 4096);
    assert_eq!(case.expected.len(), 90 * 4096);
    let after = &case.expected;
    for number in 24..89 {
        let body = original::body(after, number, 0);
        let next = u32::from_le_bytes(body[..4].try_into().unwrap());
        assert_eq!(
            next,
            if number == 88 {
                0
            } else {
                ((number + 1) << 8) as u32
            }
        );
        assert_eq!(body.len(), if number == 88 { 1540 } else { 4076 });
        assert!(
            body[4..]
                .as_chunks::<2>()
                .0
                .iter()
                .all(|pair| *pair == [0, 1])
        );
        let page = &after[number * 4096..(number + 1) * 4096];
        assert_eq!(&page[4..8], b"LVAL");
        assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 1);
        let start = usize::from(original::word(after, number, 0));
        assert_eq!(start, 4096 - body.len());
        assert_eq!(
            u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize,
            start - 16
        );
        assert!(page[16..start].iter().all(|byte| *byte == 0));
    }

    let root = original::body(after, 12, 0);
    assert_eq!(root.len(), 27);
    assert_eq!(u32::from_le_bytes(root[..4].try_into().unwrap()), 89 << 8);
    assert!(root[4..].iter().all(|byte| *byte == 0));
    assert_eq!(
        original::word(after, 12, 0),
        original::word(&case.before, 12, 0) | 0x4000
    );
    let parent = original::body(after, 89, 0);
    assert_eq!(parent.len(), 39);
    assert_eq!(
        u32::from_le_bytes(parent[6..10].try_into().unwrap()),
        262144
    );
    assert_eq!(
        u32::from_le_bytes(parent[10..14].try_into().unwrap()),
        24 << 8
    );
    assert_eq!(original::word(after, 89, 0) & 0xe000, 0x8000);

    for slot in 0..6 {
        let map = original::body(after, 11, slot);
        assert_eq!(map.len(), 69);
        assert!(map[..5] == original::body(&case.before, 11, slot)[..5]);
        for number in 0..512 {
            let member = match slot {
                0 => matches!(number, 12 | 89),
                1 => number == 89,
                2 => number == 13 || (24..89).contains(&number),
                3 => number == 88,
                4 | 5 => number == 14,
                _ => unreachable!(),
            };
            assert_eq!(map[5 + number / 8] & (1 << (number % 8)) != 0, member);
        }
    }
    let parent_page = &after[89 * 4096..90 * 4096];
    let parent_start = usize::from(original::word(after, 89, 0) & 0x1fff);
    assert!(parent_page[16..parent_start].iter().all(|byte| *byte == 0));
    let global = original::body(after, 1, 0);
    assert_eq!(global.len(), 69);
    assert!(global[..5] == original::body(&case.before, 1, 0)[..5]);
    for number in 0..512 {
        assert_eq!(
            global[5 + number / 8] & (1 << (number % 8)) != 0,
            number >= 90
        );
    }

    let mut allowed = vec![false; case.before.len()];
    for (page, slot) in [(1, 0), (11, 0), (11, 1), (11, 2), (11, 3), (11, 4), (11, 5)] {
        let start = page * 4096 + usize::from(original::word(&case.before, page, slot) & 0x1fff);
        let length = original::body(&case.before, page, slot).len();
        allowed[start..start + length].fill(true);
    }
    let root_start = 12 * 4096 + usize::from(original::word(&case.before, 12, 0) & 0x1fff);
    allowed[root_start..root_start + 27].fill(true);
    allowed[12 * 4096 + 14..12 * 4096 + 16].fill(true);
    assert!(
        case.before
            .iter()
            .zip(after)
            .zip(allowed)
            .all(|((before, after), allowed)| allowed || before == after)
    );
    original::old_chunks_unchanged(&case.before, after);
}

#[path = "write_update_files/api.rs"]
mod api;
