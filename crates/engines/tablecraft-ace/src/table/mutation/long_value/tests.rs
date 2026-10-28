//! The final certificate must be bound to the original caller's operation.
use super::{Operation, admission, finish, maps::Edit as MapEdit, stage};
use crate as tablecraft_ace;
use crate::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate, check_table,
    check_table_constraints, check_table_long_values, read_table, test_support::fixture,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

#[allow(dead_code)]
#[path = "../../../../tests/ace16_long_value_updates_contract/model.rs"]
pub(super) mod model;

use model::*;

#[test]
fn final_certificate_rejects_a_correlated_replacement_not_in_the_original_requests() {
    let before = source(6, 1);
    let requested = [SlotScalarUpdate::new(
        0,
        Value::Text("A".into()),
        Value::Text("B".into()),
    )];
    let image = capture(&before);
    let operation = Operation {
        table: "Items",
        column: "Memo",
        updates: &requested,
    };
    let prepared = admission::prepare(&image, "Items", "Memo", &mut budget(), &|| false).unwrap();
    let authorized = stage::build(&image, &prepared, &requested, &mut budget(), &|| false).unwrap();
    let healthy = finish::finish(
        capture(&before),
        &operation,
        &authorized,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        read(healthy.candidate_bytes())[0][1],
        Value::Text("B".into())
    );

    // This fully correlated staged image passes ordinary value/storage proofs,
    // but C was never assigned by the immutable original operation above.
    let unrequested = [SlotScalarUpdate::new(
        0,
        Value::Text("A".into()),
        Value::Text("C".into()),
    )];
    let forged = stage::build(&image, &prepared, &unrequested, &mut budget(), &|| false).unwrap();
    assert_eq!(
        finish::finish(
            capture(&before),
            &operation,
            &forged,
            &mut budget(),
            &|| { false },
        )
        .err(),
        Some(EditError::Preservation)
    );
}

fn exercise_after_forgery(forge: impl FnOnce(&mut stage::Staged<'_>, &[u8])) {
    let before = source(6, 1);
    let replacement = Value::Text("Ā".repeat(2039));
    let requested = [SlotScalarUpdate::new(
        0,
        Value::Text("A".into()),
        replacement.clone(),
    )];
    let operation = Operation {
        table: "Items",
        column: "Memo",
        updates: &requested,
    };
    let image = capture(&before);
    let prepared = admission::prepare(&image, "Items", "Memo", &mut budget(), &|| false).unwrap();
    let mut staged = stage::build(&image, &prepared, &requested, &mut budget(), &|| false).unwrap();
    let healthy = finish::finish(
        capture(&before),
        &operation,
        &staged,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(read(healthy.candidate_bytes())[0][1], replacement);
    forge(&mut staged, &before);

    // Establish that this is a certification failure beyond the ordinary
    // readers/checker, not just an invalid descriptor or malformed chain.
    let mut forged = before.clone();
    forged.resize(staged.plan.length, 0);
    for page in &staged.plan.pages {
        let start = page.number as usize * PAGE;
        forged[start..start + PAGE].copy_from_slice(&page.after);
    }
    for edit in &staged.plan.maps.edits {
        forged[edit.offset] = edit.after;
    }
    assert_eq!(read(&forged)[0][1], replacement);
    check(&forged);
    assert_eq!(
        finish::finish(
            capture(&before),
            &operation,
            &staged,
            &mut budget(),
            &|| false
        )
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn final_audit_preserves_old_chunk_orphan_bodies_and_zero_fresh_padding() {
    for slot in [0_usize, 1] {
        exercise_after_forgery(|staged, before| {
            let offset = usize::from(word(before, MEMO, slot) & 0x1fff);
            let page = staged
                .plan
                .pages
                .iter_mut()
                .find(|page| page.number == MEMO as u32)
                .unwrap();
            page.after[offset] ^= 1;
        });
    }
    exercise_after_forgery(|staged, _| {
        let page = staged
            .plan
            .pages
            .iter_mut()
            .find(|page| page.number == 25)
            .unwrap();
        assert_eq!(page.after[32], 0);
        page.after[32] = 0x77;
    });
}

#[test]
fn final_map_replay_rejects_an_unrelated_future_allocation_bit() {
    exercise_after_forgery(|staged, before| {
        // EOF24/25 share this original global bitmap byte with future page31.
        // Clearing31 remains reader-valid but is not an actual allocation event.
        let row_start = usize::from(word(before, 1, 0) & 0x1fff);
        let offset = PAGE + row_start + 5 + 31 / 8;
        let edit = staged
            .plan
            .maps
            .edits
            .iter_mut()
            .find(|edit| edit.offset == offset)
            .unwrap();
        assert_ne!(edit.after & 0x80, 0);
        edit.after &= !0x80;
    });
}

#[test]
fn final_map_replay_requires_each_real_failed_hint_before_the_lower_destination() {
    for remove_hint_edit in [false, true] {
        let mut before = source(6, 1);
        set_maps(&mut before, &[MEMO, 15], &[MEMO, 15], &[OLE]);
        install(&mut before, 15, &lval(&[(0, vec![0xa7; 4078])]));
        check(&before);
        let replacement = Value::Text("Ā".repeat(33));
        let updates = [memo_update(replacement.clone())];
        let operation = Operation {
            table: "Items",
            column: "Memo",
            updates: &updates,
        };
        let image = capture(&before);
        let prepared =
            admission::prepare(&image, "Items", "Memo", &mut budget(), &|| false).unwrap();
        let mut staged =
            stage::build(&image, &prepared, &updates, &mut budget(), &|| false).unwrap();
        assert_eq!(staged.plan.attempts.len(), 2);
        assert_eq!(staged.plan.attempts[0].page, 15);
        assert!(!staged.plan.attempts[0].fitted);
        assert_eq!(staged.plan.attempts[1].page, MEMO as u32);
        assert!(staged.plan.attempts[1].fitted);

        let healthy = finish::finish(
            capture(&before),
            &operation,
            &staged,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(read(healthy.candidate_bytes())[0][1], replacement);

        staged.plan.attempts.remove(0);
        if remove_hint_edit {
            // Restoring a physically full page's hint is reader-valid. It
            // cannot erase the failed attempt that chose the lower page.
            assert_eq!(staged.plan.maps.edits.len(), 1);
            staged.plan.maps.edits.clear();
        }
        assert_eq!(
            finish::finish(
                capture(&before),
                &operation,
                &staged,
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(EditError::Preservation)
        );
    }
}

#[test]
fn mixed_map_finalizer_rejects_unrequested_orphan_ownership_and_global_future_bits() {
    for claim_orphan in [false, true] {
        let mut before = source(6, 1);
        let maps = page(
            0,
            &[
                (0, map(&[ROOT])),
                (0, map(&[ROOT])),
                (0, vec![1, 19, 0, 0, 0]),
                (0, map(&[MEMO])),
                (0, map(&[OLE])),
                (0, map(&[OLE])),
            ],
        );
        install(&mut before, 11, &maps);
        let mut bitmap = vec![0; PAGE];
        bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
        bitmap[4 + 13 / 8] = 1 << (13 % 8);
        install(&mut before, 19, &bitmap);
        install(&mut before, 15, &lval(&[(0, vec![0x92, 0x04, 0x7d])]));
        check(&before);

        let replacement = Value::Text("Ā".repeat(2039));
        let updates = [memo_update(replacement.clone())];
        let operation = Operation {
            table: "Items",
            column: "Memo",
            updates: &updates,
        };
        let image = capture(&before);
        let prepared =
            admission::prepare(&image, "Items", "Memo", &mut budget(), &|| false).unwrap();
        let mut staged =
            stage::build(&image, &prepared, &updates, &mut budget(), &|| false).unwrap();
        let healthy = finish::finish(
            capture(&before),
            &operation,
            &staged,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(read(healthy.candidate_bytes())[0][1], replacement);

        if claim_orphan {
            // Claiming another allocated LVAL orphan remains reader-valid.
            // No allocation event authorizes this existing page's owned bit.
            let offset = 19 * PAGE + 4 + 15 / 8;
            staged.plan.maps.edits.push(MapEdit {
                offset,
                before: before[offset],
                after: before[offset] | (1 << (15 % 8)),
            });
        } else {
            let row_start = usize::from(word(&before, 1, 0) & 0x1fff);
            let offset = PAGE + row_start + 5 + 31 / 8;
            let byte = staged
                .plan
                .maps
                .edits
                .iter_mut()
                .find(|edit| edit.offset == offset)
                .unwrap();
            assert_ne!(byte.after & 0x80, 0);
            byte.after &= !0x80;
        }

        let mut forged = before.clone();
        forged.resize(staged.plan.length, 0);
        for page in &staged.plan.pages {
            let start = page.number as usize * PAGE;
            forged[start..start + PAGE].copy_from_slice(&page.after);
        }
        for edit in &staged.plan.maps.edits {
            forged[edit.offset] = edit.after;
        }
        assert_eq!(read(&forged)[0][1], replacement);
        check(&forged);
        assert_eq!(
            finish::finish(
                capture(&before),
                &operation,
                &staged,
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(EditError::Preservation)
        );
    }
}
