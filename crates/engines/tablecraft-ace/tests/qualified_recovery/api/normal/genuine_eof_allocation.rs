//! Real normal EOF allocation and V2 commit, using separately qualified images.
#[path = "genuine_eof_allocation/recovery.rs"]
mod recovery;

use super::{Storage, budget, resizable_storage::Target, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeUpdateIntent,
    NativeUpdateProfile, SlotScalarUpdate, qualify_native_allocation_recovery,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, V2State, inspect_v2};
use tablecraft_testkit::access::genuine_eof_allocation::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn groups() -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new(
            "Marker",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                0,
                Value::Text(sources::OLD.into()),
                Value::Text(sources::GROWN.into()),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

#[test]
#[ignore = "requires the independently read original-only EOF allocation images"]
fn original_eof_sources_commit_exact_normal_v2_images_and_confirm_without_writes() {
    for case in cases() {
        let explicit = image(&case.before)
            .plan_recoverable_ordinary_eof_updates("Sentinel", groups(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(explicit.before_bytes(), case.before);
        assert_eq!(explicit.after_bytes(), case.after);
        assert_eq!(explicit.changed_rows(), 1);
        assert_eq!(
            explicit.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryEofAllocation
        );

        let mut fixed = Storage::new(case.before.clone());
        let mut unused = Storage::new(Vec::new());
        assert!(matches!(
            explicit.prepare(&mut fixed, &mut unused, &mut budget(), &|| false),
            Err(JournalError::ImageSize)
        ));
        assert_eq!(fixed.bytes.get_ref(), &case.before);
        assert_eq!(fixed.writes, 0);
        assert_eq!(unused.writes, 0);

        let plan = image(&case.before)
            .plan_recoverable_ordinary_variable_updates(
                "Sentinel",
                groups(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryEofAllocation
        );
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        let intent = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        let mut target = Target::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare_update(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.storage.bytes.get_ref(), &case.after);
        assert_eq!(&journal.bytes.get_ref()[..8], b"TCIMG002");

        let writes = target.storage.writes;
        let resizes = target.resizes;
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_allocation_recovery(checked, intent, scope(), &mut budget(), &|| false)
                .unwrap();
        assert_eq!(qualified.state(), V2State::Committed);
        assert_eq!(
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::Committed
        );
        assert_eq!(target.storage.writes, writes);
        assert_eq!(target.resizes, resizes);
        assert_eq!(target.storage.bytes.get_ref(), &case.after);
    }
}

#[path = "genuine_eof_allocation/compound.rs"]
mod compound;
