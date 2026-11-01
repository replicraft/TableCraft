//! Original-source mixed owned/EOF reservations, qualified before these planners.
use super::{NativeFittedIntentGroup, NativeFittedUpdates, NativeUpdateProfile, SlotScalarUpdate};
use super::{Storage, Target, budget, image, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::read_table;
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_compound_eof::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
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
#[ignore = "requires all four independently qualified compound EOF source images"]
fn mixed_owned_and_eof_reservations_commit_exact_images_in_both_request_orders() {
    let all = cases();
    let mut commits = 0;
    for case in all {
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
                NativeUpdateProfile::Ace16OrdinaryEofAllocation
            );
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.changed_rows(), 2);
            let mut target = Target::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare_update(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                2
            );
            assert_eq!(target.storage.bytes.get_ref(), &case.after);
            assert_eq!(&journal.bytes.get_ref()[..8], b"TCIMG002");
            let snapshot = read_table(
                &mut Cursor::new(target.storage.bytes.get_ref().as_slice()),
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
