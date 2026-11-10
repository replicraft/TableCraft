//! Closed profile3 reconstructs nonempty rows rather than trusting stored AFTER.
use super::*;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting;

fn populated_plan(before: &[u8], replacement: &str) -> RecoverableFieldRenamePlan {
    image(before, SCOPE)
        .plan_recoverable_populated_field_rename(
            "Table1",
            "Score",
            replacement,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn populated_literal(before: &[u8], replacement: &str) -> Vec<u8> {
    let mut wire = literal(before, "Score", replacement);
    wire[12..14].copy_from_slice(&3_u16.to_le_bytes());
    wire
}

#[test]
fn explicit_populated_wire_and_fresh_requalification_preserve_all_widths_and_noop() {
    for name in ["TaxAmount", "X", "Total", "Score"] {
        let [before, after] = fitting::populated_number_pair(name, &[Some(42), None, Some(-7)]);
        let reviewed = populated_plan(&before, name);
        let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(wire, populated_literal(&before, name));
        assert_eq!(
            decode(&wire).encode(&mut budget(), &|| false).unwrap(),
            wire
        );

        let fresh = reviewed
            .requalify_with_budgets(
                image(&before, CatalogScope::new(993)),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(fresh.before_bytes(), before);
        assert_eq!(fresh.after_bytes(), after);
        assert_eq!(fresh.changed(), name != "Score");
    }
}

#[test]
fn closed_profile_tags_refuse_unknown_values_and_never_retry_other_native_families() {
    let [before, _] = fitting::populated_number_pair("Total", &[Some(42), None]);
    let good = populated_literal(&before, "Total");
    for tag in [0_u16, 5, u16::MAX] {
        let mut wire = good.clone();
        wire[12..14].copy_from_slice(&tag.to_le_bytes());
        assert!(matches!(
            NativeFieldRenameIntent::decode(&wire, &mut budget(), &|| false),
            Err(FieldRenameIntentError::InvalidRecord)
        ));
    }

    let mut old_same_width = good;
    old_same_width[12..14].copy_from_slice(&1_u16.to_le_bytes());
    assert!(
        decode(&old_same_width)
            .reconstruct(image(&before, SCOPE), &mut budget(), &|| false)
            .is_err()
    );

    let mut old_fitted = populated_literal(&before, "TaxAmount");
    old_fitted[12..14].copy_from_slice(&2_u16.to_le_bytes());
    assert!(
        decode(&old_fitted)
            .reconstruct(image(&before, SCOPE), &mut budget(), &|| false)
            .is_err()
    );

    let empty = fitting::source(fitting::SourceCase::NumberFirst);
    assert!(matches!(
        decode(&populated_literal(&empty, "Total")).reconstruct(
            image(&empty, SCOPE),
            &mut budget(),
            &|| false
        ),
        Err(FieldRenameIntentError::Rename(FieldRenameError::Source))
    ));
}

#[test]
fn pending_committed_and_forged_after_journals_reconstruct_actual_populated_source() {
    let [before, after] = fitting::populated_number_pair("X", &[Some(42), None, Some(-7)]);
    for committed in [false, true] {
        let reviewed = populated_plan(&before, "X");
        let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = reviewed
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            target.bytes.get_mut().copy_from_slice(&after);
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_field_rename_recovery(
            checked,
            decode(&wire),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            target.bytes.get_ref(),
            if committed { &after } else { &before }
        );
    }

    for offset in [8 * 4096 + 4090, 9 * 4096 + 482, 7 * 4096 + 20] {
        let mut forged = after.clone();
        forged[offset] ^= 1;
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_expected(
                &mut target,
                &mut journal,
                &before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_field_rename_recovery(
                checked,
                decode(&populated_literal(&before, "X")),
                SCOPE,
                &mut budget(),
                &|| false
            ),
            Err(FieldRenameIntentError::Proof(
                NativeRecoveryQualificationError::ImageMismatch
            ))
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}
