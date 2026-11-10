//! Cold TCNI17 recovery replays original witnesses over the same V3 owner.
use super::budget;
use crate::{
    EditError, FixedScalarUpdate, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, SlotScalarUpdate, qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_io::journal::{self, JournalError, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::mixed::after_off::deleted_field::populated::{
            Case, readdition::Target as Field,
        },
        canonical_retained_add_table::{
            Target, desktop,
            rows::{self, Input, Profile},
            updates,
        },
        canonical_table_drop::current::{self as dropped, History},
    },
    support::io::journal::MemoryStorage,
};

#[derive(Clone, Copy)]
enum Kind {
    Number,
    Text,
}

impl Kind {
    fn source(self) -> (Vec<u8>, u32) {
        let old = dropped::source(Case::MixedBinary, Field::ShortText, true, History::Deleted);
        let empty = dropped::after(&old);
        let definition = (empty.len() / 4096) as u32;
        let (target, profile, input) = match self {
            Self::Number => (
                Target::GeneratedNumber,
                Profile::DesktopNumber,
                Input::Number(42),
            ),
            Self::Text => (
                Target::GeneratedText,
                Profile::DesktopText,
                Input::Text("first"),
            ),
        };
        let created = desktop::append(&empty, "Replacement", target);
        (
            rows::append(&created, definition, profile, input),
            definition,
        )
    }

    fn values(self, noop: bool) -> (Value, Value) {
        match self {
            Self::Number => (Value::Int32(42), Value::Int32(if noop { 42 } else { -42 })),
            Self::Text => (
                Value::Text("first".into()),
                Value::Text(if noop { "first" } else { "日本語🦀" }.into()),
            ),
        }
    }

    fn groups(self, noop: bool) -> Vec<NativeFittedIntentGroup> {
        let (expected, value) = self.values(noop);
        native_groups(self, expected, value)
    }

    fn after(self, before: &[u8], definition: u32, noop: bool) -> Vec<u8> {
        match self {
            Self::Number => {
                updates::number(before, definition, 0, Some(if noop { 42 } else { -42 }))
            }
            Self::Text => updates::text(
                before,
                definition,
                0,
                Some(if noop { "first" } else { "日本語🦀" }),
            ),
        }
    }
}

fn native_groups(kind: Kind, expected: Value, value: Value) -> Vec<NativeFittedIntentGroup> {
    let (name, cells) = match kind {
        Kind::Number => (
            "Score",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(0, expected, value)]),
        ),
        Kind::Text => (
            "Name",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(0, expected, value)]),
        ),
    };
    vec![NativeFittedIntentGroup::new_retained(name, cells, &mut budget(), &|| false).unwrap()]
}

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn retained_update_requalifies_exact_original_witnesses_and_old_prepares_have_no_effects() {
    for kind in [Kind::Number, Kind::Text] {
        let (before, definition) = kind.source();
        let after = kind.after(&before, definition, false);
        let plan = image(&before, 3711)
            .plan_recoverable_retained_row_updates(
                "Replacement",
                kind.groups(false),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16RetainedRows
        );
        let original_wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        let plan = plan
            .requalify_with_budgets(image(&before, 3712), &mut budget(), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), after);
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            original_wire
        );
        let mut target = MemoryStorage::new(before.to_vec());
        let mut storage = MemoryStorage::new(Vec::new());
        assert!(matches!(
            plan.prepare(&mut target, &mut storage, &mut budget(), &|| false),
            Err(JournalError::ImageSize)
        ));
        assert_eq!((target.writes, storage.writes), (0, 0));
        assert!(storage.bytes.get_ref().is_empty());

        let plan = image(&before, 3713)
            .plan_recoverable_retained_row_updates(
                "Replacement",
                kind.groups(false),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(matches!(
            plan.prepare_update(&mut target, &mut storage, &mut budget(), &|| false),
            Err(JournalError::ImageSize)
        ));
        assert_eq!((target.writes, storage.writes), (0, 0));
        assert_eq!(target.bytes.get_ref(), &before);
        assert!(storage.bytes.get_ref().is_empty());
    }
}

#[test]
fn retained_update_pending_and_committed_v3_recovery_and_confirmation_include_noops() {
    for kind in [Kind::Number, Kind::Text] {
        let (before, definition) = kind.source();
        for noop in [false, true] {
            let after = kind.after(&before, definition, noop);
            if noop {
                assert_eq!(after, before);
            }
            for committed in [false, true] {
                let plan = image(&before, 3714)
                    .plan_recoverable_retained_row_updates(
                        "Replacement",
                        kind.groups(noop),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.after_bytes(), after);
                let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
                let mut target = MemoryStorage::new(before.to_vec());
                let mut storage = MemoryStorage::new(Vec::new());
                let prepared = plan
                    .prepare_retained(&mut target, &mut storage, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    assert_eq!(
                        prepared.commit(&mut budget(), &|| false).unwrap(),
                        usize::from(!noop)
                    );
                } else {
                    drop(prepared);
                    target.bytes.get_mut().copy_from_slice(&after);
                }
                assert!(journal::inspect_v1(&mut storage, &mut budget(), &|| false).is_err());
                assert!(journal::inspect_v2(&mut storage, &mut budget(), &|| false).is_err());
                let checked = journal::inspect_v3(&mut storage, &mut budget(), &|| false).unwrap();
                let qualified = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(3715),
                        &mut budget(),
                        &|| false,
                    )
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
                    if committed { &after } else { &before }
                );
                let writes = target.writes;
                let checked = journal::inspect_v3(&mut storage, &mut budget(), &|| false).unwrap();
                NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(3716),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(target.writes, writes);
            }
        }
    }
}

#[test]
fn retained_update_refuses_stale_expected_forged_after_and_cross_family_authority() {
    let kind = Kind::Text;
    let (before, definition) = kind.source();
    assert!(matches!(
        image(&before, 3717).plan_recoverable_retained_row_updates(
            "Replacement",
            native_groups(kind, Value::Text("stale".into()), Value::Null),
            &mut budget(),
            &|| false
        ),
        Err(RecoverableEditError::Edit(EditError::Conflict))
    ));
    let plan = image(&before, 3718)
        .plan_recoverable_retained_row_updates(
            "Replacement",
            kind.groups(false),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
    let mut forged = kind.after(&before, definition, false);
    forged[7 * 4096 + 100] ^= 1;
    let mut target = MemoryStorage::new(before.to_vec());
    let mut storage = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut storage,
            &before,
            &forged,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut storage, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(3719), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);

    let mut legacy_wire = wire.to_vec();
    legacy_wire[6..8].copy_from_slice(b"16");
    legacy_wire[12..14].copy_from_slice(&16_u16.to_le_bytes());
    let checked = journal::inspect_v3(&mut storage, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&legacy_wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(3720), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedProfile
        ))
    ));
    let mut old_storage = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut old_storage,
            &before,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut old_storage, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_update_recovery(
            checked,
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(3721),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedProfile
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
