#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ace16_long_value_updates_contract/model.rs"]
mod model;
#[allow(dead_code)]
mod support;

use model::*;
use std::cell::Cell;
use std::io::Cursor;
use tablecraft_ace::{
    Ace16WritePreview, EditError, InspectError, LongValueCheckStatus, NativeImage,
    SlotScalarUpdate, WriteBlocker, check_table, check_table_constraints, check_table_long_values,
    read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn original(code: u8, compressed: bool) -> Vec<u8> {
    let mut bytes = source(code, 1);
    bytes[10 * PAGE + 104] = u8::from(compressed);
    let wire = if compressed {
        vec![0xff, 0xfe, 0x41, 0x42, 0x43]
    } else {
        vec![0x41, 0, 0x42, 0, 0x43, 0]
    };
    install(
        &mut bytes,
        MEMO,
        &lval(&[(0, wire.clone()), (0, vec![0xf1, 0xe2, 0xd3])]),
    );
    let row = record(
        1,
        Some(&single(wire.len(), MEMO, 0)),
        Some(&single(1, OLE, 0)),
    );
    install(&mut bytes, ROOT, &page(10, &[(0, row)]));
    assert_eq!(read(&bytes)[0][1], Value::Text("ABC".into()));
    assert!(
        check(&bytes)
            .columns()
            .iter()
            .all(|column| column.status() == LongValueCheckStatus::Checked)
    );
    bytes
}

fn wire(bytes: &[u8]) -> Vec<u8> {
    let mut row = body(bytes, ROOT, 0);
    if word(bytes, ROOT, 0) & 0x4000 != 0 {
        let reference = u32::from_le_bytes(row[..4].try_into().unwrap());
        row = body(bytes, (reference >> 8) as usize, (reference & 255) as usize);
    }
    let start = usize::from(u16::from_le_bytes(
        row[row.len() - 5..row.len() - 3].try_into().unwrap(),
    ));
    let end = usize::from(u16::from_le_bytes(
        row[row.len() - 7..row.len() - 5].try_into().unwrap(),
    ));
    let descriptor = &row[start..end];
    let tagged = u32::from_le_bytes(descriptor[..4].try_into().unwrap());
    if tagged & 0x8000_0000 != 0 {
        return descriptor[12..].to_vec();
    }
    let mut reference = u32::from_le_bytes(descriptor[4..8].try_into().unwrap());
    let mut result = Vec::new();
    while reference != 0 {
        let chunk = body(bytes, (reference >> 8) as usize, (reference & 255) as usize);
        if tagged & 0x4000_0000 != 0 {
            result.extend_from_slice(chunk);
            break;
        }
        reference = u32::from_le_bytes(chunk[..4].try_into().unwrap());
        result.extend_from_slice(&chunk[4..]);
    }
    assert_eq!(result.len(), (tagged & 0x3fff_ffff) as usize);
    result
}

fn replace(bytes: &[u8], value: &str) -> Result<Ace16WritePreview, EditError> {
    run(
        bytes,
        "Memo",
        &[SlotScalarUpdate::new(
            0,
            Value::Text("ABC".into()),
            Value::Text(value.into()),
        )],
    )
}

#[test]
fn observed_ties_latin1_nul_wide_nonbmp_and_markers_match_literal_wire() {
    let cases = [
        ("", vec![]),
        ("A", vec![0x41, 0]),
        ("AB", vec![0x41, 0, 0x42, 0]),
        ("\u{1}AB", vec![0xff, 0xfe, 1, 0x41, 0x42]),
        ("éÿß", vec![0xff, 0xfe, 0xe9, 0xff, 0xdf]),
        ("A\0B", vec![0x41, 0, 0, 0, 0x42, 0]),
        ("AĀB", vec![0x41, 0, 0, 1, 0x42, 0]),
        ("A😀B", vec![0x41, 0, 0x3d, 0xd8, 0, 0xde, 0x42, 0]),
        ("\u{fffe}AB", vec![0xfe, 0xff, 0x41, 0, 0x42, 0]),
    ];
    for code in [5, 6] {
        let before = original(code, true);
        for (value, expected) in &cases {
            let candidate = replace(&before, value).unwrap();
            assert_eq!(
                read(candidate.candidate_bytes())[0][1],
                Value::Text((*value).into())
            );
            assert_eq!(wire(candidate.candidate_bytes()), *expected);
            old_chunks_unchanged(&before, candidate.candidate_bytes());
            assert_eq!(candidate.candidate_bytes()[10 * PAGE + 104], 1);
        }
        for value in ["\u{feff}", "\u{feff}AB"] {
            assert_eq!(
                replace(&before, value).err(),
                Some(EditError::Unsupported(WriteBlocker::Column))
            );
        }
        let noop = replace(&before, "ABC").unwrap();
        assert_eq!(noop.candidate_bytes(), before);
    }
}

#[test]
fn independent_1023_1024_1025_boundaries_preserve_raw_sibling_policy() {
    for code in [5, 6] {
        for compressed in [false, true] {
            let before = original(code, compressed);
            for (length, narrow_bytes) in [(1023, 1025), (1024, 1026), (1025, 2050)] {
                for (character, low) in [('A', 0x41), ('é', 0xe9)] {
                    let value = character.to_string().repeat(length);
                    let candidate = replace(&before, &value).unwrap();
                    let actual = wire(candidate.candidate_bytes());
                    let expected = if compressed && length != 1025 {
                        let mut expected = vec![0xff, 0xfe];
                        expected.extend(vec![low; length]);
                        assert_eq!(expected.len(), narrow_bytes);
                        expected
                    } else {
                        [low, 0].repeat(length)
                    };
                    assert_eq!(actual, expected);
                    assert_eq!(read(candidate.candidate_bytes())[0][1], Value::Text(value));
                }
            }
        }
    }
}

#[test]
fn unknown_history_calculated_and_ole_compression_flags_remain_closed() {
    for code in [5, 6] {
        let before = original(code, true);
        for flags in [2, 3, 0x10, 0x80, 0xc0, 0xc1] {
            let mut changed = before.clone();
            changed[10 * PAGE + 104] = flags;
            assert!(run(&changed, "Memo", &[]).is_err());
        }
        let mut changed = before.clone();
        changed[10 * PAGE + 129] = 1;
        assert_eq!(
            run(&changed, "Ole", &[]).err(),
            Some(EditError::Inspect(InspectError::Unsupported))
        );
        let stored = check_table_long_values(
            &mut Cursor::new(&changed),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            stored.columns()[1].status(),
            LongValueCheckStatus::UnsupportedColumn
        );
    }
}

#[test]
fn compressed_requests_keep_stale_quota_cancel_and_retry_guards() {
    let before = original(6, true);
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Text("ABC".into()),
        Value::Text("é".repeat(1024)),
    )];
    for limits in [
        ResourceBudget::new(0, 1000, 1_000_000),
        ResourceBudget::new(1_000_000, 0, 1_000_000),
        ResourceBudget::new(1_000_000, 1000, 0),
    ] {
        let mut limits = limits;
        assert!(
            capture(&before)
                .preview_ace16_long_value_updates("Items", "Memo", &updates, &mut limits, &|| false)
                .is_err()
        );
    }
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() >= 20
    };
    assert_eq!(
        capture(&before)
            .preview_ace16_long_value_updates("Items", "Memo", &updates, &mut budget(), &cancel)
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    let stale = [SlotScalarUpdate::new(
        0,
        Value::Text("ABD".into()),
        Value::Text("ééé".into()),
    )];
    assert!(run(&before, "Memo", &stale).is_err());
    let first = run(&before, "Memo", &updates).unwrap();
    let retry = run(&before, "Memo", &updates).unwrap();
    assert_eq!(first.candidate_bytes(), retry.candidate_bytes());
}

#[test]
fn compressed_storage_does_not_bypass_properties_or_logical_len_constraints() {
    for code in [5, 6] {
        let mut before = source(code, 1);
        before[10 * PAGE + 104] = 1;
        table_rule(&mut before, "Len([Memo]) <= 1");
        check(&before);
        assert_eq!(
            run(&before, "Memo", &[memo_update(Value::Text("ééé".into()))]).err(),
            Some(EditError::Inspect(InspectError::Constraint(
                tablecraft_ace::ConstraintViolation::TableRule
            )))
        );
        let candidate = run(&before, "Memo", &[memo_update(Value::Text("B".into()))]).unwrap();
        assert_eq!(
            read(candidate.candidate_bytes())[0][1],
            Value::Text("B".into())
        );

        let mut property = original(code, true);
        selected_property(&mut property, "Caption");
        assert!(run(&property, "Memo", &[]).is_err());
        assert!(replace(&property, "ééé").is_err());

        // A valid raw representation under a compressed declaration remains
        // byte-exact when the requested semantic value is unchanged.
        let mut noncanonical = original(code, false);
        noncanonical[10 * PAGE + 104] = 1;
        let noop = replace(&noncanonical, "ABC").unwrap();
        assert_eq!(noop.candidate_bytes(), noncanonical);
    }
}
