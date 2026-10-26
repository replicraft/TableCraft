use super::model::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, IndexCheckStatus, InspectError, Int32Update, RowDelete, WriteBlocker, check_table,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn statuses(model: &Model) -> Vec<IndexCheckStatus> {
    check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .indexes()
    .iter()
    .map(|index| index.status())
    .collect()
}

#[test]
fn read_profiles_keep_delete_codec_and_update_gates_separate() {
    let mut ignored = text(1, false, false);
    ignored.bytes[10 * 4096 + 189] = 0x82;
    let entries = ignored
        .order
        .iter()
        .copied()
        .filter(|row| *row != 0)
        .map(|row| wire(&ignored, row))
        .collect::<Vec<_>>();
    ignored.bytes[13 * 4096..14 * 4096].copy_from_slice(&page(0, &entries, 0, 0, 0));
    assert!(
        statuses(&ignored)
            .iter()
            .all(|status| *status == IndexCheckStatus::Checked)
    );
    assert_eq!(
        candidate(&ignored, &[]).unwrap().candidate_bytes(),
        ignored.bytes
    );
    let omitted = candidate(&ignored, &[0]).unwrap();
    assert_eq!(omitted.deleted_rows(), 1);
    assert_eq!(
        &omitted.candidate_bytes()[13 * 4096..14 * 4096],
        &ignored.bytes[13 * 4096..14 * 4096]
    );
    assert_eq!(omitted.candidate_bytes()[10 * 4096 + 189], 0x82);
    for updates in [vec![], vec![Int32Update::new(1, 2, 3)]] {
        assert_eq!(
            capture(&ignored.bytes)
                .preview_ace16_int32_updates("Items", "Id", &updates, &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
    }
    for (model, expected) in [
        (
            null_memo(),
            EditError::Unsupported(WriteBlocker::IndexedColumn),
        ),
        (composite(), EditError::Inspect(InspectError::Unsupported)),
    ] {
        assert!(
            statuses(&model)
                .iter()
                .all(|status| *status == IndexCheckStatus::Checked)
        );
        for deleted in [vec![], vec![0]] {
            assert_eq!(candidate(&model, &deleted).err().as_ref(), Some(&expected));
        }
    }
    let mut unknown = text(1, false, false);
    unknown.bytes[10 * 4096 + 111..10 * 4096 + 115].copy_from_slice(&[31, 4, 0, 1]);
    for model in [unknown, single_overflow()] {
        assert!(statuses(&model).contains(&IndexCheckStatus::Unsupported));
        assert!(matches!(
            candidate(&model, &[]),
            Err(EditError::Unsupported(_))
        ));
        assert!(matches!(
            candidate(&model, &[0]),
            Err(EditError::Unsupported(_))
        ));
    }
}

#[test]
fn expected_payload_or_encoded_row_disagreement_never_publishes_a_candidate() {
    for model in [text(1, false, true), binary(true, true)] {
        let mut stale = model.records[2].clone();
        stale[2] = 99;
        assert_eq!(
            capture(&model.bytes)
                .preview_ace16_deletes("Items", &[RowDelete::new(2, stale)], &mut budget(), &|| {
                    false
                })
                .err(),
            Some(EditError::Conflict)
        );
        let row = model.order[0];
        let original = wire(&model, row);
        let mut corrupt = model.bytes.clone();
        corrupt[13 * 4096 + 480 + original.len() - 1] ^= 1;
        assert!(
            capture(&corrupt)
                .preview_ace16_deletes("Items", &requests(&model, &[2]), &mut budget(), &|| false)
                .is_err()
        );
        assert!(
            capture(&corrupt)
                .preview_ace16_deletes("Items", &[], &mut budget(), &|| false)
                .is_err()
        );
        assert!(candidate(&model, &[2]).is_ok());
    }
}

#[test]
fn retained_wire_clones_share_quotas_and_cancel_without_a_candidate_or_source_change() {
    let model = tree(text(1, true, true), true);
    let deleted = model.order[..3].to_vec();
    let requests = requests(&model, &deleted);
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            capture(&model.bytes).preview_ace16_deletes(
                "Items",
                &requests,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0usize);
    let expected = capture(&model.bytes)
        .preview_ace16_deletes("Items", &requests, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let observed = calls.get();
    assert!(observed > 8);
    for checkpoint in [
        0,
        1,
        observed / 4,
        observed / 2,
        observed * 3 / 4,
        observed - 1,
    ] {
        let calls = Cell::new(0usize);
        let result =
            capture(&model.bytes).preview_ace16_deletes("Items", &requests, &mut budget(), &|| {
                let current = calls.get();
                calls.set(current + 1);
                current == checkpoint
            });
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Cancelled))),
            "checkpoint {checkpoint}/{observed}"
        );
    }
    assert_eq!(
        candidate(&model, &deleted).unwrap().candidate_bytes(),
        expected.candidate_bytes()
    );
    assert_eq!(expected.before_bytes(), model.bytes);
    verify_rows(&model, expected.candidate_bytes(), &deleted);
}
