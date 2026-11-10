//! Distinct populated-tail requests rebuild complete images before recovery.
use super::*;
use raw::populated;

fn current_plan(before: &[u8], column: &str) -> RecoverableFieldDropPlan {
    image(before, SCOPE)
        .plan_recoverable_field_drop("Table1", column, &mut budget(), &|| false)
        .unwrap()
}

fn current_literal(before: &[u8], case: populated::Case) -> Vec<u8> {
    let mut wire = literal(before, populated::column(case));
    wire[..8].copy_from_slice(b"TCFD0002");
    wire[12..14].copy_from_slice(&2_u16.to_le_bytes());
    let identity = populated::identity(case);
    wire[64..66].copy_from_slice(&identity.native_number.to_le_bytes());
    wire[66..68].copy_from_slice(&identity.column_id.to_le_bytes());
    wire
}

#[test]
fn populated_tail_wire_and_fresh_scope_keep_original_identities_and_legacy_language_closed() {
    for case in populated::CASES {
        let [before, after] = populated::pair(case);
        let column = populated::column(case);
        let expected_identity = populated::identity(case);
        let reviewed = current_plan(&before, column);
        assert_eq!(
            reviewed.identity().column_number(),
            usize::from(expected_identity.native_number)
        );
        assert_eq!(reviewed.identity().column_id(), expected_identity.column_id);
        assert!(reviewed.after_bytes() == after);
        let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(wire, current_literal(&before, case));
        assert!(matches!(
            NativeFieldDropIntent::decode(&wire, &mut budget(), &|| false),
            Err(FieldDropIntentError::InvalidRecord)
        ));
        let decoded =
            NativeFieldDropIntent::decode_current_tail(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
        let fresh = reviewed
            .requalify_with_budgets(
                image(&before, CatalogScope::new(994)),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(fresh.after_bytes() == after);
        assert_eq!(fresh.identity().column_id(), expected_identity.column_id);

        let mut crossed = current_literal(&before, case);
        crossed[..8].copy_from_slice(b"TCFD0001");
        crossed[12..14].copy_from_slice(&1_u16.to_le_bytes());
        let legacy = NativeFieldDropIntent::decode(&crossed, &mut budget(), &|| false).unwrap();
        assert!(
            legacy
                .reconstruct(image(&before, SCOPE), &mut budget(), &|| false)
                .is_err()
        );
        assert!(
            NativeFieldDropIntent::decode_current_tail(&crossed, &mut budget(), &|| false).is_err()
        );
    }
}

#[test]
fn populated_tail_cold_recovery_rejects_forged_afters_and_restores_complete_images() {
    for case in populated::CASES {
        let [before, after] = populated::pair(case);
        let column = populated::column(case);
        for committed in [false, true] {
            // NO_CLONE: Recovery mutates owned storage while BEFORE remains the independent complete byte oracle.
            let mut target = MemoryStorage::new(before.clone());
            let mut log = MemoryStorage::new(Vec::new());
            let prepared = current_plan(&before, column)
                .prepare(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut()[7 * 4096..8 * 4096]
                    .copy_from_slice(&after[7 * 4096..8 * 4096]);
            }
            let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            let intent = NativeFieldDropIntent::decode_current_tail(
                &current_literal(&before, case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            qualify_native_field_drop_recovery(
                checked,
                intent,
                CatalogScope::new(995),
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
            assert!(target.bytes.get_ref() == if committed { &after } else { &before });
        }
        // NO_CLONE: Forge one candidate byte while retaining the independent expected image.
        let mut forged = after.clone();
        *forged.last_mut().unwrap() ^= 1;
        // NO_CLONE: Generic journal preparation requires owned mutable storage; BEFORE remains the refusal oracle.
        let mut target = MemoryStorage::new(before.clone());
        let mut log = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_expected(
                &mut target,
                &mut log,
                &before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
        let intent = NativeFieldDropIntent::decode_current_tail(
            &current_literal(&before, case),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            qualify_native_field_drop_recovery(checked, intent, SCOPE, &mut budget(), &|| false)
                .is_err()
        );
        assert_eq!(target.writes, 0);
        assert!(target.bytes.get_ref() == &before);
    }
}

#[test]
fn populated_tail_header_and_identity_guards_precede_native_reconstruction() {
    let case = populated::Case::TextPresent;
    let before = populated::source(case);
    for offset in [0, 8, 12, 14, 72] {
        let mut wire = current_literal(&before, case);
        wire[offset] ^= 1;
        assert!(matches!(
            NativeFieldDropIntent::decode_current_tail(
                &wire,
                &mut ResourceBudget::new(0, 0, 10_000),
                &|| false,
            ),
            Err(FieldDropIntentError::InvalidRecord)
        ));
    }
    let mut wire = current_literal(&before, case);
    wire[66..68].copy_from_slice(&999_u16.to_le_bytes());
    let intent =
        NativeFieldDropIntent::decode_current_tail(&wire, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        intent.reconstruct(image(&before, SCOPE), &mut budget(), &|| false),
        Err(FieldDropIntentError::IdentityMismatch)
    ));
    let valid = current_literal(&before, case);
    assert!(matches!(
        NativeFieldDropIntent::decode_current_tail(&valid, &mut budget(), &|| true),
        Err(FieldDropIntentError::Inspect(InspectError::Cancelled))
    ));
}
