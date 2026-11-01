//! Source-qualified two-root batch reservations; no staged bytes supply goldens.
#[path = "genuine_owned_growth_multirow/fixtures.rs"]
mod fixtures;
#[path = "genuine_owned_growth_multirow/recovery.rs"]
mod recovery;
#[path = "genuine_owned_growth_multirow/wire.rs"]
mod wire;

use super::{Storage, budget, model as journals, scope};
use fixtures::cases;
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeUpdateProfile,
    RecoverableEditError, SlotScalarUpdate, WriteBlocker, read_table,
};
use tablecraft_core::values::Value;

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(reverse: bool) -> Vec<NativeFittedIntentGroup> {
    let mut updates = vec![
        SlotScalarUpdate::new(
            0,
            Value::Text("unchanged sentinel".into()),
            Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
        ),
        SlotScalarUpdate::new(
            1,
            Value::Text("unchanged sentinel".into()),
            Value::Text("ccccccccccccccccccc".into()),
        ),
    ];
    if reverse {
        updates.reverse();
    }
    vec![
        NativeFittedIntentGroup::new(
            "Marker",
            NativeFittedUpdates::Variable(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

#[test]
#[ignore = "requires all six source-qualified multirow owned images"]
fn two_original_rows_reserve_distinct_pages_in_native_order_and_commit_exact_images() {
    let mut commits = 0;
    for case in cases() {
        for reverse in [false, true] {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_variable_updates(
                    "Sentinel",
                    groups(reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryOwnedGrowth
            );
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire::encode(&case, reverse)
            );
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.changed_rows(), 2);
            let changed = case
                .before
                .as_chunks::<4096>()
                .0
                .iter()
                .zip(case.after.as_chunks::<4096>().0.iter())
                .enumerate()
                .filter_map(|(page, (before, after))| (before != after).then_some(page as u32))
                .collect::<Vec<_>>();
            assert_eq!(changed, case.pages);
            assert_eq!(plan.before_bytes().len(), plan.after_bytes().len());
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                2
            );
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, Marker::Committed)
            );
            let snapshot = read_table(
                &mut Cursor::new(target.bytes.get_ref().as_slice()),
                scope(),
                "Sentinel",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                snapshot.rows(),
                [
                    vec![
                        Value::Int32(23),
                        Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
                        Value::Binary(vec![199, 0, 255])
                    ],
                    vec![
                        Value::Int32(24),
                        Value::Text("ccccccccccccccccccc".into()),
                        Value::Binary(vec![199, 0, 255])
                    ],
                ]
            );
            commits += 1;
        }
    }
    assert_eq!(commits, 4);
}

#[test]
#[ignore = "requires all six source-qualified multirow owned images"]
fn healthy_no_capacity_source_refuses_second_reservation_without_external_effects() {
    for case in cases() {
        let source = image(&case.no_capacity);
        source
            .check_native_ordinary_fitted_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        let first = case.pages[1] as usize * 4096;
        let second = case.pages[2] as usize * 4096;
        assert_eq!(&case.no_capacity[first + 12..first + 14], &[254, 0]);
        assert_eq!(&case.no_capacity[first + 2..first + 4], &[246, 13]);
        assert_eq!(&case.no_capacity[second + 12..second + 14], &[255, 0]);
        assert_eq!(&case.no_capacity[second + 2..second + 4], &[244, 13]);
        for reverse in [false, true] {
            let target = Storage::new(case.no_capacity.clone());
            let journal = Storage::new(Vec::new());
            let result = image(target.bytes.get_ref()).plan_recoverable_ordinary_variable_updates(
                "Sentinel",
                groups(reverse),
                &mut budget(),
                &|| false,
            );
            assert!(matches!(
                result,
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::RowLayout
                )))
            ));
            assert_eq!(target.writes, 0);
            assert_eq!(target.bytes.get_ref(), &case.no_capacity);
            assert_eq!(journal.writes, 0);
            assert!(journal.bytes.get_ref().is_empty());
        }
    }
}
