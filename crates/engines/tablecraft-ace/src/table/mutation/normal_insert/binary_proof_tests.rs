//! Literal source plans only; no stage, serializer or normal planner is called.
use super::indexed::proof;
use crate::{EditError, NativeEditPlan, RowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_binary_indexed as sources;

const SCOPE: CatalogScope = CatalogScope::new(217);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<sources::Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn requests(case: &sources::Case) -> Vec<RowInsert> {
    sources::insert_values(case.recipe)
        .into_iter()
        .map(RowInsert::new)
        .collect()
}

fn plan(case: &sources::Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone().into(),
        after: case.after.clone(),
        changed_rows: sources::insert_values(case.recipe).len(),
    }
}

fn check(plan: &NativeEditPlan, rows: &[RowInsert]) -> Result<(), EditError> {
    proof::prove(plan, SCOPE, "BinaryKeys", rows, &mut budget(), &|| false)
}

fn entry_spans(raw: &[u8], root: u32) -> Vec<(usize, usize)> {
    let base = root as usize * 4096;
    let mut result = Vec::new();
    let mut start = base + 480;
    // Literal source bitmap/active area geometry, independent of the parser.
    for bit in 0..453 * 8 {
        if raw[base + 27 + bit / 8] & (1 << (bit % 8)) != 0 {
            let end = base + 480 + bit;
            assert!(end > start && end <= base + 4096);
            result.push((start, end));
            start = end;
        }
    }
    result
}

fn forgery_offsets(case: &sources::Case) -> Vec<usize> {
    let td = case.definition as usize * 4096;
    let descending = entry_spans(&case.after, case.roots[2]);
    let continued = descending
        .iter()
        .find_map(|(start, end)| {
            // Present components longer than one 9B segment retain the literal
            // unflipped intermediate09 tag even in DESC order.
            if end - start > 14 {
                let at = start + 9;
                assert_eq!(case.after[at], 9);
                Some(at)
            } else {
                None
            }
        })
        .unwrap();
    let padding = descending
        .iter()
        .find_map(|(start, end)| {
            let component = end - start - 4;
            if component <= 1 {
                return None;
            }
            assert_eq!((component - 1) % 9, 0);
            let final_start = start + 1 + (component - 10);
            let used = usize::from(!case.after[end - 5]);
            assert!((1..=8).contains(&used));
            if used == 8 {
                None
            } else {
                let at = final_start + used;
                assert_eq!(case.after[at], 255);
                Some(at)
            }
        })
        .unwrap();
    let name = "AscendingAlias"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let aliases = case.after[td..td + 4096]
        .windows(name.len())
        .enumerate()
        .filter_map(|(at, value)| (value == name.as_slice()).then_some(td + at))
        .collect::<Vec<_>>();
    assert_eq!(aliases.len(), 1);
    let first = sources::insert_values(case.recipe);
    let length = match &first[0][1] {
        Value::Null => 0,
        Value::Binary(bytes) => bytes.len(),
        _ => panic!("outside frozen source profile"),
    };
    let new_start = case.data as usize * 4096 + 3816 - 17 - length;
    let directory = case.data as usize * 4096 + 40;
    let actual_start = usize::from(u16::from_le_bytes(
        case.after[directory..directory + 2].try_into().unwrap(),
    ));
    assert_eq!(actual_start, new_start % 4096);
    vec![
        td + 16,
        td + 67 + 12,
        case.roots[0] as usize * 4096 + 480 + 8,
        case.roots[1] as usize * 4096 + 27,
        case.roots[1] as usize * 4096 + 480,
        continued,
        padding,
        aliases[0],
        new_start + 17 + length - 1,
        case.data as usize * 4096 + 4095,
        case.roots[2] as usize * 4096 + 4095,
    ]
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified Binary source views"]
fn literal_binary_plans_prove_without_producer_and_reject_raw_causal_forgeries() {
    let all = cases();
    let mut passed = 0;
    let mut refused = 0;
    for case in &all {
        let literal = plan(case);
        let rows = requests(case);
        check(&literal, &rows).unwrap();
        passed += 1;
        for at in forgery_offsets(case) {
            let mut forged = plan(case);
            forged.after[at] ^= 1;
            assert_eq!(check(&forged, &rows), Err(EditError::Preservation));
            refused += 1;
        }
        let mut count = plan(case);
        count.changed_rows += 1;
        assert_eq!(check(&count, &rows), Err(EditError::Preservation));
        refused += 1;
    }
    assert_eq!(passed, 16);
    assert_eq!(refused, 192);
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified Binary source views"]
fn binary_null_empty_and_batch_request_order_are_part_of_the_original_proof() {
    let mut refused = 0;
    for case in cases() {
        if matches!(case.recipe, "insert-null" | "insert-empty") {
            let mut values = sources::insert_values(case.recipe);
            values[0][1] = if case.recipe == "insert-null" {
                Value::Binary(Vec::new())
            } else {
                Value::Null
            };
            let rows = values.into_iter().map(RowInsert::new).collect::<Vec<_>>();
            assert_eq!(check(&plan(&case), &rows), Err(EditError::Preservation));
            refused += 1;
        }
        if case.recipe == "batch" {
            let mut rows = requests(&case);
            rows.reverse();
            assert_eq!(check(&plan(&case), &rows), Err(EditError::Preservation));
            refused += 1;
        }
    }
    assert_eq!(refused, 6);
}
