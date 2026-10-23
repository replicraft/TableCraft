#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
mod support;
use std::{cell::Cell, io::Cursor};
use support::{data_page, put16, put32};
use tablecraft_ace::{
    InspectError, TableCursor, read_table, read_table_batched, read_table_projection,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const SCOPE: CatalogScope = CatalogScope::new(1);
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn old_row(count: u16) -> Vec<u8> {
    let mut row = count.to_le_bytes().to_vec();
    row.extend(i32::from(count).to_le_bytes());
    if count == 1 {
        row.push(0xff);
        return row;
    }
    row.extend(999_i32.to_le_bytes()); // Deleted fixed payload must never become a value.
    if count == 6 {
        row.extend(600_i32.to_le_bytes());
    }
    let start = row.len() as u16;
    row.extend([b'x', 0]); // Deleted variable payload occupies its historical slot.
    let keep = row.len() as u16;
    if (4..6).contains(&count) {
        row.extend([b'y', 0]);
    }
    let end = row.len() as u16;
    row.extend(end.to_le_bytes());
    if count >= 4 {
        row.extend(keep.to_le_bytes());
    }
    row.extend(start.to_le_bytes());
    row.extend((if count >= 4 { 2_u16 } else { 1_u16 }).to_le_bytes());
    row.push(match count {
        3 => 7,
        4 => 0xff,
        5 => 0x1f,
        6 => 0x27,
        _ => unreachable!(),
    });
    row
}
fn database() -> Vec<u8> {
    let mut bytes = fixture::database();
    let mut definition = fixture::definition(
        &[
            ("Id", 4, 0, 4),
            ("Keep", 10, 0, 40),
            ("Flag", 1, 0, 1),
            ("Counter", 4, 8, 4),
        ],
        5,
        11 << 8,
    );
    put16(&mut definition, 41, 6);
    put16(&mut definition, 43, 2);
    for (slot, number) in [0, 3, 4, 5].into_iter().enumerate() {
        put16(
            &mut definition,
            FIELD_START + slot * FIELD_BYTES + 5,
            number,
        );
    }
    put16(&mut definition, FIELD_START + FIELD_BYTES + 7, 1);
    bytes[DEFINITION_PAGE * PAGE_BYTES..(DEFINITION_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(&data_page(
        DEFINITION_PAGE as u32,
        &[1, 3, 4, 5, 6]
            .into_iter()
            .map(|count| (0, old_row(count)))
            .collect::<Vec<_>>(),
    ));
    bytes
}

#[test]
fn deleted_fixed_and_variable_identities_preserve_all_read_paths_in_each_profile() {
    let expected = [
        vec![
            Value::Int32(1),
            Value::Null,
            Value::Boolean(false),
            Value::Null,
        ],
        vec![
            Value::Int32(3),
            Value::Null,
            Value::Boolean(false),
            Value::Null,
        ],
        vec![
            Value::Int32(4),
            Value::Text("y".into()),
            Value::Boolean(false),
            Value::Null,
        ],
        vec![
            Value::Int32(5),
            Value::Text("y".into()),
            Value::Boolean(true),
            Value::Null,
        ],
        vec![
            Value::Int32(6),
            Value::Null,
            Value::Boolean(false),
            Value::Int32(600),
        ],
    ];
    for code in [1, 2, 5, 6] {
        let mut bytes = database();
        bytes[20] = code;
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        assert_eq!(
            read_table(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            expected
        );
        assert_eq!(
            read_table_batched(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            expected
        );
        let mut input = Cursor::new(&bytes);
        let mut cursor =
            TableCursor::open(&mut input, SCOPE, "Items", &mut budget(), &|| false).unwrap();
        for row in &expected {
            assert_eq!(
                cursor
                    .next_page(1, &mut budget(), &|| false)
                    .unwrap()
                    .rows(),
                std::slice::from_ref(row)
            );
        }
        let projected = read_table_projection(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &["Id", "Counter"],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for (row, expected) in projected.rows().iter().zip(&expected) {
            assert_eq!(row, &[expected[0].clone(), expected[3].clone()]);
        }
    }
}

fn prefix_database(variable_tail: bool) -> Vec<u8> {
    let mut bytes = database();
    put32(&mut bytes, DEFINITION_PAGE * PAGE_BYTES + 16, 1);
    let mut row = vec![2, 0, 2, 0, 0, 0];
    if variable_tail {
        // The removed field was variable: one UTF-16 unit and a complete tail.
        row.extend([b'x', 0, 8, 0, 6, 0, 1, 0]);
    } else {
        // The removed field was fixed: its bytes do not describe a tail.
        row.extend([9, 0, 0, 0]);
    }
    // Unused high bits cannot make future visible columns present.
    row.push(0xff);
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(DEFINITION_PAGE as u32, &[(0, row)]));
    bytes
}

#[test]
fn known_fixed_values_do_not_require_inventing_deleted_tail_types() {
    let expected = [vec![
        Value::Int32(2),
        Value::Null,
        Value::Boolean(false),
        Value::Null,
    ]];
    for code in [1, 2, 5, 6] {
        for variable_tail in [false, true] {
            let mut bytes = prefix_database(variable_tail);
            bytes[20] = code;
            if code != 1 {
                bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
            }
            assert_eq!(
                read_table(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                expected
            );
            assert_eq!(
                read_table_batched(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                expected
            );
            assert_eq!(
                read_table_projection(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    &["Id"],
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                [vec![Value::Int32(2)]]
            );
            assert_eq!(
                read_table_projection(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    &[],
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                [vec![]]
            );
        }
    }
}

#[test]
fn historical_fixed_prefix_still_checks_counts_mask_bounds_and_known_payloads() {
    for row in [
        vec![],
        vec![2],
        vec![2, 0],
        vec![2, 0, 2, 0, 0, 0],
        vec![0, 0, 0],
        vec![7, 0, 0],
    ] {
        let mut bytes = prefix_database(false);
        bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&data_page(DEFINITION_PAGE as u32, &[(0, row)]));
        for selected in [&["Id"][..], &[][..]] {
            assert!(
                read_table_projection(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    selected,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
    }
}

#[test]
fn historical_prefix_reads_retain_limits_and_every_cancellation_checkpoint() {
    let bytes = prefix_database(true);
    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        read_table(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &cancelled,
        )
    };
    assert!(invoke(usize::MAX).is_ok());
    let observed = calls.get();
    for stop in 1..=observed {
        assert!(matches!(invoke(stop), Err(InspectError::Cancelled)));
    }
    let mut exhausted = ResourceBudget::new(0, 0, 0);
    assert!(
        read_table(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut exhausted,
            &|| false
        )
        .is_err()
    );
}

#[test]
fn impossible_tail_counts_and_duplicate_variable_slots_do_not_decode() {
    for variables in [0, 1, 3] {
        let mut bytes = database();
        put32(&mut bytes, DEFINITION_PAGE * PAGE_BYTES + 16, 1);
        let mut row = old_row(4);
        let at = row.len() - 3;
        put16(&mut row, at, variables);
        bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&data_page(DEFINITION_PAGE as u32, &[(0, row)]));
        assert!(
            read_table(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let mut bytes = fixture::database();
    let mut definition =
        fixture::definition(&[("First", 10, 0, 40), ("Second", 10, 0, 40)], 0, 11 << 8);
    put16(&mut definition, FIELD_START + FIELD_BYTES + 7, 0);
    bytes[DEFINITION_PAGE * PAGE_BYTES..(DEFINITION_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);
    assert!(matches!(
        read_table(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Malformed)
    ));
}

#[test]
fn a_required_tail_uses_its_stored_count_within_deleted_identity_bounds() {
    for deleted_variable_was_present in [false, true] {
        let mut bytes = fixture::database();
        let mut definition = fixture::definition(
            &[("Id", 4, 0, 4), ("Keep", 10, 0, 40), ("Counter", 4, 8, 4)],
            1,
            11 << 8,
        );
        put16(&mut definition, 41, 5);
        put16(&mut definition, 43, 2);
        put16(&mut definition, FIELD_START + 2 * FIELD_BYTES + 5, 4);
        bytes[DEFINITION_PAGE * PAGE_BYTES..(DEFINITION_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&definition);
        let mut row = 3_u16.to_le_bytes().to_vec();
        row.extend(3_i32.to_le_bytes());
        if !deleted_variable_was_present {
            row.extend(999_i32.to_le_bytes());
        }
        let start = row.len() as u16;
        row.extend([b'y', 0]);
        let keep_end = row.len() as u16;
        if deleted_variable_was_present {
            row.extend([b'z', 0]);
            let end = row.len() as u16;
            row.extend(end.to_le_bytes());
            row.extend(keep_end.to_le_bytes());
        } else {
            row.extend(keep_end.to_le_bytes());
        }
        row.extend(start.to_le_bytes());
        row.extend(
            (if deleted_variable_was_present {
                2_u16
            } else {
                1_u16
            })
            .to_le_bytes(),
        );
        row.push(7);
        bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&data_page(DEFINITION_PAGE as u32, &[(0, row)]));
        let table = read_table(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            table.rows(),
            &[vec![Value::Int32(3), Value::Text("y".into()), Value::Null]]
        );
    }
}

#[test]
fn newly_readable_history_does_not_qualify_even_current_row_writes() {
    use tablecraft_ace::{EditError, Int32Update, NativeImage, WriteBlocker};
    let bytes = database();
    let image =
        NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        image.plan_int32_updates(
            "Items",
            "Counter",
            &[Int32Update::new(4, 600, 601)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    for variable_tail in [false, true] {
        let bytes = prefix_database(variable_tail);
        let image = NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            image.plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(0, 2, 3)],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
    }
}
