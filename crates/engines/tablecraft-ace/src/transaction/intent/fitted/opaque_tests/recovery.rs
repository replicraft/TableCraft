//! TCNI18 cold replay keeps opaque source proof separate from complete-row17.
use super::{budget, literal_column};
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeImage, NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent,
    NativeUpdateProfile, RecoverableEditError, RecoverableNativeEditPlan,
    qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_io::journal::{self, JournalError, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::opaque_rows as raw,
    support::io::journal::MemoryStorage,
};

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn groups(value: Option<i32>) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained(
            "NextNumber",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                0,
                Value::Null,
                value.map_or(Value::Null, Value::Int32),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn plan(before: &[u8], value: Option<i32>) -> RecoverableNativeEditPlan {
    image(before, 1630)
        .plan_recoverable_retained_opaque_row_updates(
            "Table1",
            groups(value),
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn opaque_wire_rebuilds_original_tail_witness_in_fresh_scope_and_old_prepares_refuse() {
    for case in raw::CASES {
        let source = raw::source(case);
        let after = raw::after(&source.bytes, source.definition, source.roots[0], Some(-42));
        let reviewed = plan(&source.bytes, Some(-42));
        assert_eq!(
            reviewed.intent().profile(),
            NativeUpdateProfile::Ace16RetainedOpaqueRows
        );
        assert_eq!(
            reviewed.intent().encode(&mut budget(), &|| false).unwrap(),
            literal_column(&source.bytes, 0, "NextNumber", None, Some(-42))
        );
        assert_eq!(reviewed.after_bytes(), after);
        let fresh = reviewed
            .requalify_with_budgets(
                image(&source.bytes, 1631),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(fresh.after_bytes(), after);
        assert_eq!(fresh.changed_rows(), 1);
        assert!(matches!(
            image(&source.bytes, 1632).plan_recoverable_retained_row_updates(
                "Table1",
                groups(Some(-42)),
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Inspect(
                InspectError::Unsupported
            )))
        ));
        let mut target = MemoryStorage::new(source.bytes.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        assert!(matches!(
            fresh.prepare(&mut target, &mut journal, &mut budget(), &|| false),
            Err(JournalError::ImageSize)
        ));
        assert!(matches!(
            plan(&source.bytes, Some(-42)).prepare_update(
                &mut target,
                &mut journal,
                &mut budget(),
                &|| false
            ),
            Err(JournalError::ImageSize)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(journal.writes, 0);
    }
}

#[test]
fn opaque_changed_and_noop_requests_recover_and_confirm_the_same_v3_journal() {
    let source = raw::source(raw::Case::VariableRetired);
    for value in [Some(-42), None] {
        let after = if value.is_some() {
            raw::after(&source.bytes, source.definition, source.roots[0], value)
        } else {
            source.bytes.clone()
        };
        let wire = literal_column(&source.bytes, 0, "NextNumber", None, value);
        for committed in [false, true] {
            let mut target = MemoryStorage::new(source.bytes.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            let prepared = plan(&source.bytes, value)
                .prepare_retained(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&after);
            }
            let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1633), &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(
                qualified.state(),
                if committed {
                    ResizableState::Committed
                } else {
                    ResizableState::Pending
                }
            );
            qualified
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &after } else { &source.bytes }
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1634), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
}

#[test]
fn opaque_recovery_refuses_forged_after_and_cross_profile_authority_without_target_writes() {
    let source = raw::source(raw::Case::VariableRetired);
    let after = raw::after(&source.bytes, source.definition, source.roots[0], Some(-42));
    let wire = literal_column(&source.bytes, 0, "NextNumber", None, Some(-42));
    let mut forged = after.clone();
    *forged.last_mut().unwrap() ^= 1;
    let mut target = MemoryStorage::new(source.bytes.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal,
            &source.bytes,
            &forged,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1635), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);

    let mut old_wire = wire.clone();
    old_wire[..8].copy_from_slice(b"TCNI0017");
    old_wire[12..14].copy_from_slice(&17_u16.to_le_bytes());
    let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&old_wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1635), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(EditError::Inspect(
            InspectError::Unsupported
        )))
    ));
    assert_eq!(target.writes, 0);

    let mut original = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut original,
            &source.bytes,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut original, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_update_recovery(
            checked,
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(1635),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedProfile
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &source.bytes);
}
