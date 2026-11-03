//! Existing fixed I32 planning beside newly certified readonly GUID indexes.
use super::SCOPE;
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage,
    NativeUpdateProfile, WriteBlocker, inspect_table_schema,
    table::mutation::no_required_programs::NoRequiredTablePrograms,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_testkit::fixtures::access::genuine_indexed_guid_update::raw::{
    ORIGINAL_PINS, PAGE, geometry, guid, indexed_records,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn capture(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "same four pinned originals; I32 plan only, no GUID mutation or journal"]
fn original_score_plan_preserves_every_guid_owner_and_old_target_policy() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_INDEX_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let images: Vec<_> = ORIGINAL_PINS
        .iter()
        .filter(|(_, direction, _, _)| direction.is_some())
        .map(|&(header, _, name, hash)| {
            let raw = fs::read(directory.join(name)).unwrap();
            let actual = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(actual, hash);
            (header, raw)
        })
        .collect();

    for (header, raw) in &images {
        let references = indexed_records(raw, *header);
        let (_, data, _) = geometry(*header);
        assert_eq!(references[0].0, (data[0] as u32) << 8);
        assert_eq!(&references[0].1[22..26], &(-64_i32).to_le_bytes());

        // Independent original-only expectation is materialized before any
        // real planner call, never derived from its returned byte image.
        let mut expected = raw.clone();
        let score = data[0] * PAGE + 4069 + 22;
        expected[score..score + 4].copy_from_slice(&(-63_i32).to_le_bytes());

        let original = capture(raw);
        let schema = inspect_table_schema(
            &mut original.reader(),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            NoRequiredTablePrograms::capture_actions(&original, &schema, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::ActiveContent)),
        ));

        let changed = [FixedScalarUpdate::new(
            0,
            Value::Guid(guid(0)),
            Value::Guid(guid(1)),
        )];
        let equal = [FixedScalarUpdate::new(
            0,
            Value::Guid(guid(0)),
            Value::Guid(guid(0)),
        )];
        for updates in [&changed[..], &equal[..], &[]] {
            let groups = [FixedColumnUpdates::new("Items", "Group", updates)];
            assert!(matches!(
                capture(raw)
                    .plan_ordinary_fixed_updates("Items", &groups, &mut budget(), &|| false),
                Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
            ));
        }
        let empty = [FixedColumnUpdates::new("Items", "Group", &[])];
        assert!(matches!(
            capture(raw).plan_ordinary_ieee_updates("Items", &empty, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        ));
        assert!(matches!(
            capture(raw).plan_ordinary_scalar_updates("Items", &empty, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        ));

        let original = NativeFixedIntentGroup::new(
            "Score",
            vec![FixedScalarUpdate::new(
                0,
                Value::Int32(-64),
                Value::Int32(-63),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let planned = capture(raw)
            .plan_recoverable_ordinary_fixed_updates(
                "Items",
                vec![original],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            planned.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryFixed
        );
        assert_eq!(planned.changed_rows(), 1);
        assert_eq!(planned.before_bytes(), raw);
        assert_eq!(planned.after_bytes(), expected);
    }
    assert_eq!(images.len(), 4);
}
