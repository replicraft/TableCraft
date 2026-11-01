#![cfg(feature = "journal")]

use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    Int32Update, NativeImage, NativeInt32EditIntent, NativeIntentError, NativeUpdateIntent,
    NativeUpdateProfile,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_before_change::{Recipe, source};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn ace_original_request_record_matches_independent_literal_versioned_framing() {
    for header in [5, 6] {
        let case = source(header, Recipe::RestoreOriginal);
        let mut work = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            CatalogScope::new(92),
            &mut work,
            &|| false,
        )
        .unwrap();
        let plan = image
            .plan_recoverable_int32_updates(
                "Items",
                "Score",
                vec![Int32Update::new(0, 10, 17)],
                &mut work,
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.changed_rows(), 0);
        assert_eq!(plan.before_bytes(), plan.after_bytes());

        // Independent data format: 72-byte fixed header, ten name bytes and
        // one 16-byte original request. No producer codec constants are used.
        let mut expected = Vec::from(*b"TCNI0002");
        expected.extend_from_slice(&98_u32.to_le_bytes());
        expected.extend_from_slice(&2_u16.to_le_bytes());
        expected.extend_from_slice(&0_u16.to_le_bytes());
        expected.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
        let mut hash = Sha256::new();
        hash.update(b"TCNI0001:BEFORE\0");
        hash.update((case.before.len() as u64).to_le_bytes());
        hash.update(&case.before);
        expected.extend_from_slice(&hash.finalize());
        expected.extend_from_slice(&5_u32.to_le_bytes());
        expected.extend_from_slice(&5_u32.to_le_bytes());
        expected.extend_from_slice(&1_u32.to_le_bytes());
        expected.extend_from_slice(&0_u32.to_le_bytes());
        expected.extend_from_slice(b"ItemsScore");
        expected.extend_from_slice(&0_u64.to_le_bytes());
        expected.extend_from_slice(&10_i32.to_le_bytes());
        expected.extend_from_slice(&17_i32.to_le_bytes());
        assert_eq!(expected.len(), 98);
        assert_eq!(
            plan.intent().encode(&mut work, &|| false).unwrap(),
            expected
        );

        let decoded = NativeUpdateIntent::decode(&expected, &mut work, &|| false).unwrap();
        assert_eq!(
            decoded.profile(),
            NativeUpdateProfile::Ace16BeforeChangeInt32
        );
        assert_eq!(decoded.table(), "Items");
        assert_eq!(decoded.int32().expect("Int32 intent").column(), "Score");
        assert_eq!(decoded.int32().expect("Int32 intent").updates().len(), 1);
        assert_eq!(
            decoded.int32().expect("Int32 intent").updates()[0].value(),
            17
        );
        decoded
            .verify_before(&case.before, &mut work, &|| false)
            .unwrap();
        assert!(matches!(
            NativeInt32EditIntent::decode(&expected, &mut work, &|| false),
            Err(NativeIntentError::UnsupportedVersion),
        ));

        let mut wrong_profile = expected.clone();
        wrong_profile[12..14].copy_from_slice(&1_u16.to_le_bytes());
        assert!(matches!(
            NativeUpdateIntent::decode(&wrong_profile, &mut work, &|| false),
            Err(NativeIntentError::UnsupportedProfile),
        ));
        let mut unknown = expected;
        unknown[..8].copy_from_slice(b"TCNI0004");
        assert!(matches!(
            NativeUpdateIntent::decode(&unknown, &mut work, &|| false),
            Err(NativeIntentError::UnsupportedVersion),
        ));
    }
}
