#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_ace::{EditError, Int32Update, NativeImage, WriteBlocker, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}
fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
// Synthetic layout tests, not actual ACE-authored/native compatibility evidence.
fn ace(mut bytes: Vec<u8>, code: u8) -> Vec<u8> {
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes
}

#[test]
fn preview_preserves_unrelated_bytes_and_normal_ace_writes_remain_blocked() {
    for code in [5, 6] {
        let bytes = ace(fixture::with_rule("ValidationRule", ">=0 And <=100"), code);
        let update = [Int32Update::new(1, 20, 27)];
        assert_eq!(
            capture(&bytes)
                .plan_int32_updates("Items", "Score", &update, &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Format))
        );
        let preview = capture(&bytes)
            .preview_ace16_int32_updates("Items", "Score", &update, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(preview.before_bytes(), bytes);
        assert_eq!(preview.changed_rows(), 1);
        let differences = bytes
            .iter()
            .zip(preview.candidate_bytes())
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect::<Vec<_>>();
        assert_eq!(differences.len(), 1);
        assert_eq!(bytes[differences[0]], 20);
        assert_eq!(preview.candidate_bytes()[differences[0]], 27);
        let rows = read_table(
            &mut Cursor::new(preview.candidate_bytes()),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rows.rows()[1][1], Value::Int32(27));
        let no_op = capture(&bytes)
            .preview_ace16_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 20)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(no_op.changed_rows(), 0);
        assert_eq!(no_op.before_bytes(), no_op.candidate_bytes());
    }
}

#[test]
fn preview_retains_rule_conflict_property_cancellation_and_resource_gates() {
    let bytes = ace(fixture::with_rule("ValidationRule", ">=0 And <=100"), 6);
    for updates in [
        vec![Int32Update::new(1, 99, 27)],
        vec![Int32Update::new(1, 20, 27), Int32Update::new(1, 20, 28)],
    ] {
        assert_eq!(
            capture(&bytes)
                .preview_ace16_int32_updates("Items", "Score", &updates, &mut budget(), &|| false)
                .err(),
            Some(EditError::Conflict)
        );
    }
    assert!(
        capture(&bytes)
            .preview_ace16_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 101)],
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    for (mut allowance, cancelled) in [(budget(), true), (ResourceBudget::new(0, 0, 0), false)] {
        assert!(
            capture(&bytes)
                .preview_ace16_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(1, 20, 27)],
                    &mut allowance,
                    &|| cancelled
                )
                .is_err()
        );
    }
    let unknown = ace(fixture::with_rule("UnqualifiedBehavior", "anything"), 6);
    assert_eq!(
        capture(&unknown)
            .preview_ace16_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 27)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Properties))
    );
    for bytes in [fixture::database(), ace(fixture::database(), 2)] {
        assert_eq!(
            capture(&bytes)
                .preview_ace16_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Format))
        );
    }
}

#[test]
fn preview_reorders_qualified_unique_indexes_and_rejects_duplicate_keys() {
    for code in [5, 6] {
        for original in [
            index_fixture::database(false, false, false),
            index_fixture::database(true, false, false),
            index_fixture::node_database(),
        ] {
            let bytes = ace(original, code);
            let preview = capture(&bytes)
                .preview_ace16_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(1, 20, 35)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let check = check_table(
                &mut Cursor::new(preview.candidate_bytes()),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                check.indexes()[0].status(),
                tablecraft_ace::IndexCheckStatus::Checked
            );
            assert_eq!(
                capture(&bytes)
                    .preview_ace16_int32_updates(
                        "Items",
                        "Score",
                        &[Int32Update::new(1, 20, 10)],
                        &mut budget(),
                        &|| false
                    )
                    .err(),
                Some(EditError::Inspect(
                    tablecraft_ace::InspectError::IndexConstraint(
                        tablecraft_ace::IndexConstraintViolation::DuplicateKey
                    )
                ))
            );
        }
    }
}

#[test]
fn known_opaque_behavior_blocks_even_an_empty_preview() {
    let mut bytes = ace(fixture::database(), 6);
    // Independent third MSysObjects slot, LvExtra presence carrier. The opaque
    // bytes must never be decoded just to block this engineering operation.
    let start = usize::from(u16::from_le_bytes(
        bytes[4 * 4096 + 18..4 * 4096 + 20].try_into().unwrap(),
    )) & 0x1fff;
    let end = usize::from(u16::from_le_bytes(
        bytes[4 * 4096 + 16..4 * 4096 + 18].try_into().unwrap(),
    )) & 0x1fff;
    bytes[4 * 4096 + start..4 * 4096 + start + 2].copy_from_slice(&8_u16.to_le_bytes());
    bytes[4 * 4096 + end - 1] |= 0x80;
    assert_eq!(
        capture(&bytes)
            .preview_ace16_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::ActiveContent))
    );
}
