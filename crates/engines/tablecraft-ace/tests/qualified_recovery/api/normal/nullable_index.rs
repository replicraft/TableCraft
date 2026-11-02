//! Existing qualified_recovery::api::normal child; old source route on restart.
use super::{Storage, budget, intent, model as journals, scope};
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{
    Int32Update, NativeImage, NativeUpdateProfile, qualify_native_update_recovery,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::canonical_nullable_index_before_change as model;

#[test]
fn nullable_unchanged_index_original_intent_requalifies_pending_and_committed_journals() {
    for header in [5, 6] {
        let case = model::source(header);
        let requests = || {
            vec![Int32Update::new(
                case.update.ordinal,
                case.update.expected,
                case.update.replacement,
            )]
        };
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let plan = image
            .plan_recoverable_int32_updates("Items", "Score", requests(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), case.expected);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16BeforeChangeInt32
        );
        assert_eq!(
            plan.intent().int32().expect("Int32 intent").updates()[0].value(),
            17
        );
        for marker in [Marker::Pending, Marker::Committed] {
            let committed = matches!(marker, Marker::Committed);
            let mut journal = Storage::new(journals::journal(&case.before, &case.expected, marker));
            let saved = journal.bytes.get_ref().clone();
            let mut target = Storage::new(case.expected.clone());
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_update_recovery(
                checked,
                intent(&case.before, requests()),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified
                    .recover(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.bytes.get_ref(),
                if committed {
                    &case.expected
                } else {
                    &case.before
                }
            );
            assert_eq!(
                &target.bytes.get_ref()[9 * 4096..],
                &case.before[9 * 4096..]
            );
            if committed {
                assert_eq!(target.writes, 0);
            }
            let writes = target.writes;
            target.fail_prefix = Some(0);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_update_recovery(
                checked,
                intent(&case.before, requests()),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(target.writes, writes);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}
