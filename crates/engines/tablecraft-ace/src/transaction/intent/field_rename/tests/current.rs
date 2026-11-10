//! Current metadata has a separate request language and cold reconstruction proof.
use super::*;
use tablecraft_testkit::fixtures::access::{
    canonical_initial_table_creation::deletes,
    saved_field_rename::fitting::{self, SourceCase},
};

fn current_plan(before: &[u8], original: &str, replacement: &str) -> RecoverableFieldRenamePlan {
    image(before, SCOPE)
        .plan_recoverable_current_field_rename(
            "Table1",
            original,
            replacement,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn current_literal(before: &[u8], original: &str, replacement: &str) -> Vec<u8> {
    let mut wire = literal(before, original, replacement);
    wire[12..14].copy_from_slice(&4_u16.to_le_bytes());
    wire
}

#[test]
fn current_requests_requalify_cumulative_metadata_instead_of_inventing_initial_history() {
    for (case, values) in [
        (SourceCase::NumberFirst, &[][..]),
        (SourceCase::NumberHistory, &[][..]),
        (SourceCase::NumberFirst, &[Some(42), None, Some(-7)][..]),
    ] {
        let mut before = if values.is_empty() {
            fitting::source(case)
        } else {
            let [before, _] = fitting::populated_number_pair("Score", values);
            before
        };
        let mut original = "Score";
        for replacement in ["TaxAmount", "X", "Revenue", "Score", "Score", "score"] {
            let after = fitting::rename_current(&before, original, replacement);
            let reviewed = current_plan(&before, original, replacement);
            let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(wire, current_literal(&before, original, replacement));
            assert_eq!(
                decode(&wire).encode(&mut budget(), &|| false).unwrap(),
                wire
            );
            let fresh = reviewed
                .requalify_with_budgets(
                    image(&before, CatalogScope::new(994)),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(fresh.before_bytes(), before);
            assert_eq!(fresh.after_bytes(), after);
            assert_eq!(fresh.changed(), original != replacement);
            before = after;
            original = replacement;
        }
    }
}

#[test]
fn current_profile_never_reinterprets_legacy_requests_or_unknown_tags() {
    let [_, grown] = fitting::populated_number_pair("TaxAmount", &[Some(42), None]);
    let before = fitting::rename_current(&grown, "TaxAmount", "X");
    for (tag, replacement) in [(1_u16, "Y"), (2, "Revenue"), (3, "Revenue")] {
        let mut wire = current_literal(&before, "X", replacement);
        wire[12..14].copy_from_slice(&tag.to_le_bytes());
        assert!(
            decode(&wire)
                .reconstruct(image(&before, SCOPE), &mut budget(), &|| false)
                .is_err(),
            "legacy profile {tag} must retain its original source proof"
        );
    }
    for tag in [0_u16, 6, u16::MAX] {
        let mut wire = current_literal(&before, "X", "Revenue");
        wire[12..14].copy_from_slice(&tag.to_le_bytes());
        assert!(matches!(
            NativeFieldRenameIntent::decode(&wire, &mut budget(), &|| false),
            Err(FieldRenameIntentError::InvalidRecord)
        ));
    }
}

#[test]
fn current_journals_rederive_renamed_tombstoned_source_and_refuse_forged_after_bytes() {
    let [_, grown] =
        fitting::populated_number_pair("TaxAmount", &[Some(99), None, Some(-7), Some(0)]);
    let shrunk = fitting::rename_current(&grown, "TaxAmount", "X");
    let before = deletes::long_row(&shrunk, 1);
    let after = fitting::rename_current(&before, "X", "Revenue");
    let wire = current_literal(&before, "X", "Revenue");
    for committed in [false, true] {
        let reviewed = current_plan(&before, "X", "Revenue");
        assert_eq!(reviewed.after_bytes(), after);
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
        qualify_native_field_rename_recovery(checked, decode(&wire), SCOPE, &mut budget(), &|| {
            false
        })
        .unwrap()
        .recover(&mut target, &mut budget(), &|| false)
        .unwrap();
        assert_eq!(
            target.bytes.get_ref(),
            if committed { &after } else { &before }
        );
    }

    // Valid journal digests cannot authorize altered row data, keys, counters or
    // opaque definition history outside the metadata operation's exact scope.
    for offset in [
        8 * 4096 + 4090,
        9 * 4096 + 482,
        7 * 4096 + 20,
        7 * 4096 + 1000,
    ] {
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
                decode(&wire),
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
