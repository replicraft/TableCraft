#![cfg(feature = "journal")]
use std::io::Cursor;
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeUpdateIntent,
    SlotScalarUpdate, qualify_native_update_recovery,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_large_memo, support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn groups(ordinal: usize) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new(
            "Score",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                ordinal,
                Value::Text("old".into()),
                Value::Text("new".into()),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

#[test]
fn ordinary_inline_memo_update_reconstructs_large_complete_source() {
    for (count, ordinal) in [(3, 0), (16_385, 0), (16_385, 16_384)] {
        let source = canonical_large_memo::source(count, ordinal);
        let plan = image(&source.before)
            .plan_recoverable_ordinary_fitted_updates(
                "Items",
                groups(ordinal),
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("count {count}: {error:?}"));
        assert_eq!(plan.before_bytes(), source.before);
        assert_eq!(plan.after_bytes(), source.after);
        assert_eq!(plan.changed_rows(), 1);
        let reviewed = plan
            .requalify(image(&source.before), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(reviewed.after_bytes(), source.after);

        let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        let mut target = MemoryStorage::new(source.before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = reviewed
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        drop(prepared);
        assert_eq!(target.bytes.get_ref(), &source.before);
        *target.bytes.get_mut() = source.after.clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let original = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_update_recovery(
            checked,
            original,
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::RolledBack
        );
        assert_eq!(target.bytes.get_ref(), &source.before);
    }
}

#[test]
fn large_memo_preparation_rejects_explicit_row_exhaustion_and_late_foreign_data() {
    let source = canonical_large_memo::source(16_385, 0);
    let mut limited = ResourceBudget::new(1_000_000_000, 16_384, 1_000_000_000);
    assert!(
        image(&source.before)
            .plan_recoverable_ordinary_fitted_updates("Items", groups(0), &mut limited, &|| false)
            .is_err()
    );
    for foreign_owner in [false, true] {
        let mut corrupt = source.before.clone();
        if foreign_owner {
            let owner = source.last_page * 4096 + 4;
            corrupt[owner..owner + 4].copy_from_slice(&0_u32.to_le_bytes());
        } else {
            // The final logical row 16,384 is unselected. Its ordinary Memo
            // still must decode: an unpaired high surrogate cannot be ignored.
            let directory = source.last_page * 4096 + 14 + 2 * (16_384 % 120);
            let start =
                u16::from_le_bytes(corrupt[directory..directory + 2].try_into().unwrap()) as usize;
            let text = source.last_page * 4096 + start + 18;
            corrupt[text..text + 2].copy_from_slice(&0xd800_u16.to_le_bytes());
        }
        assert!(
            image(&corrupt)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    groups(0),
                    &mut budget(),
                    &|| { false }
                )
                .is_err()
        );
    }
}

#[test]
fn huge_data_only_ordinal_cannot_authorize_an_out_of_source_native_edit() {
    let source = canonical_large_memo::source(3, 0);
    let mut allowance = ResourceBudget::new(8 * 1024 * 1024, 10_000, 100_000_000);
    assert!(matches!(
        image(&source.before).plan_recoverable_ordinary_fitted_updates(
            "Items",
            groups(usize::MAX),
            &mut allowance,
            &|| false
        ),
        Err(tablecraft_ace::RecoverableEditError::Edit(
            tablecraft_ace::EditError::Conflict
        ))
    ));
}
