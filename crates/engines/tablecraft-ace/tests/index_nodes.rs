#[path = "mutation/fixture.rs"]
mod base;
#[path = "composite_indexes/fixture.rs"]
mod fixture;
#[path = "index_nodes/limits.rs"]
mod limits;
mod support;

use std::{
    io::Cursor,
    ops::Bound::{Included, Unbounded},
};
use tablecraft_ace::{
    EditError, IndexCheckStatus, InspectError, Int32IndexSelection, Int32Update, NativeImage,
    WriteBlocker, check_table, read_table_index, read_table_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn database() -> Vec<u8> {
    let mut bytes = fixture::database(
        6,
        &[4, 4],
        &[(0, false)],
        &[vec![Some(1), Some(7)], vec![Some(2), Some(8)]],
        0,
    );
    fixture::index_flags(&mut bytes, 0x81);
    bytes.resize(17 * 4096, 0);
    bytes[13 * 4096..14 * 4096].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 1])],
    ));
    for (number, leaf, keys, children, prev, next) in [
        (
            14,
            false,
            vec![(1_i32, 0_u32), (2, 1)],
            vec![15_u32, 16],
            0_u32,
            0_u32,
        ),
        (15, true, vec![(1, 0)], vec![], 0, 16),
        (16, true, vec![(2, 1)], vec![], 15, 0),
    ] {
        let page = &mut bytes[number * 4096..(number + 1) * 4096];
        page.fill(0);
        page[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
        support::put32(page, 4, 10);
        support::put32(page, 12, prev);
        support::put32(page, 16, next);
        page[26] = u8::from(!leaf);
        let mut end = 0;
        for (position, (key, slot)) in keys.into_iter().enumerate() {
            let mut entry = vec![0x7f];
            entry.extend(((key as u32) ^ 0x80000000).to_be_bytes());
            entry.extend(((12_u32 << 8) | slot).to_be_bytes());
            if !leaf {
                entry.extend(children[position].to_be_bytes());
            }
            page[480 + end..480 + end + entry.len()].copy_from_slice(&entry);
            end += entry.len();
            page[27 + end / 8] |= 1 << (end % 8);
        }
        support::put16(page, 2, (4096 - 480 - end) as u16);
    }
    bytes
}

#[test]
fn explicit_last_child_bounds_need_no_tail_for_full_reads_checks_and_seeks() {
    let bytes = database();
    let table = read_table_index(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(7)],
            vec![Value::Int32(2), Value::Int32(8)]
        ]
    );
    assert_eq!(
        check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .indexes()[0]
            .status(),
        IndexCheckStatus::Checked
    );
    let range = read_table_index_range(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        Int32IndexSelection::Range {
            lower: Included(2),
            upper: Unbounded,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(range.rows(), &[vec![Value::Int32(2), Value::Int32(8)]]);
}

#[test]
fn missing_all_children_duplicate_tails_and_nonzero_leaf_tails_remain_malformed() {
    for change in 0..3 {
        let mut bytes = database();
        match change {
            0 => {
                bytes[14 * 4096 + 27..15 * 4096].fill(0);
                support::put16(&mut bytes, 14 * 4096 + 2, 3616);
            }
            1 => support::put32(&mut bytes, 14 * 4096 + 20, 15),
            _ => support::put32(&mut bytes, 15 * 4096 + 20, 16),
        }
        assert_eq!(
            read_table_index(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Malformed)
        );
    }
}

#[test]
fn explicit_last_child_topologies_stay_outside_key_mutation() {
    let mut bytes = database();
    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    bytes[20] = 1;
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        image
            .plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(0, 1, 3)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
}
