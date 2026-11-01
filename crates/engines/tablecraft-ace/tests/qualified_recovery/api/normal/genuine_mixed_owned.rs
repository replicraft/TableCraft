//! One destination page receives both cross-page and same-page growth events.
use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Cursor, path::PathBuf};
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeUpdateProfile,
    SlotScalarUpdate,
};
use tablecraft_core::values::Value;

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires the independently qualified mixed-event source images"]
fn mixed_owned_events_preserve_both_roots_and_commit_exact_independent_images() {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_MIXED_OWNED_EXPECTED_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, before_sha, after_sha) in [
        (
            5,
            "9654761272b4e1fbc76a4b2a8d3a7c86b7bcb1f3db9d8f7d320bd289ceab8ced",
            "6b12cfbbc1838b33cdbe30f9e95707354e9f9fe982d06289088c5d7719d4b49c",
        ),
        (
            6,
            "f3ad302be80519b73f9283a3a045b051191b42c96eb2e03fcb57e18f3243f5ae",
            "921ba8be0a8d45e3e849f99e3a6a74fe3d5bc59a409540028a339a2ae6313d1d",
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-mixed-events-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-mixed-events-growth.accdb"))).unwrap();
        verify(&before, before_sha);
        verify(&after, after_sha);
        cases.push((before, after));
    }

    // Qualify every source pin before invoking any producer.
    for (before, after) in cases {
        let updates = ["aaaaaaaaaaaaaaaaaaa", "ccccccccccccccccccc"]
            .into_iter()
            .enumerate()
            .map(|(ordinal, replacement)| {
                SlotScalarUpdate::new(
                    ordinal,
                    Value::Text("unchanged sentinel".into()),
                    Value::Text(replacement.into()),
                )
            })
            .collect();
        let groups = vec![
            NativeFittedIntentGroup::new(
                "Marker",
                NativeFittedUpdates::Variable(updates),
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        ];
        let image =
            NativeImage::capture(&mut Cursor::new(&before), scope(), &mut budget(), &|| false)
                .unwrap();
        let plan = image
            .plan_recoverable_ordinary_variable_updates("Sentinel", groups, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), after);
        assert_eq!(plan.changed_rows(), 2);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryOwnedGrowth
        );

        let mut target = Storage::new(before);
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            2
        );
        assert_eq!(target.bytes.get_ref(), &after);
    }
}
