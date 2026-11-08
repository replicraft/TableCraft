//! Described canonical initialization replays its original text through TCNT5 only.
use super::{Storage, Target, budget, intent, pending, scope};
use std::io::Cursor;
use tablecraft_ace::{
    CreateColumn, FirstTableDefinition, NativeImage, NativeRecoveryQualificationError,
    PropertyGroupKind, inspect_table_properties, qualify_native_first_table_recovery,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v2};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

#[test]
fn described_first_table_commit_cold_recovery_and_tamper_keep_exact_metadata() {
    let before = canonical_blank_creation::source();
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap()
            .with_description("")
            .unwrap(),
        CreateColumn::short_text("Name", 20)
            .unwrap()
            .with_description("Résumé 🗃 help")
            .unwrap(),
    ];
    let make_plan = || {
        let definition =
            FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
        NativeImage::capture(&mut Cursor::new(&before), scope(), &mut budget(), &|| false)
            .unwrap()
            .plan_first_table(definition, &mut budget(), &|| false)
            .unwrap()
    };
    let plan = make_plan();
    let after = plan.after_bytes().to_vec();
    let raw = plan.encode_intent(&mut budget(), &|| false).unwrap();
    assert_eq!(&raw[..8], b"TCNT0005");
    let properties = inspect_table_properties(
        &mut Cursor::new(&after),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for (field, text) in [("Id", ""), ("Name", "Résumé 🗃 help")] {
        let property = properties
            .group(PropertyGroupKind::Column, field)
            .unwrap()
            .get("Description")
            .unwrap();
        assert_eq!(property.native_type(), 10);
        assert_eq!(property.text().unwrap(), text);
    }
    for committed in [false, true] {
        let mut target = Target::new(before.clone());
        let mut journal = if committed {
            Storage::new(Vec::new())
        } else {
            pending(&before, &after)
        };
        if committed {
            make_plan()
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.storage.bytes.get_ref(), &after);
        }
        let mut cold = Storage::new(journal.bytes.get_ref().clone());
        let checked = inspect_v2(&mut cold, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_first_table_recovery(
            checked,
            intent(&raw),
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
            target.storage.bytes.get_ref(),
            if committed { &after } else { &before }
        );
    }
    let mut changed = raw;
    *changed.last_mut().unwrap() = b'x';
    let mut journal = pending(&before, &after);
    let writes = journal.writes;
    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_first_table_recovery(
            checked,
            intent(&changed),
            scope(),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(journal.writes, writes);
}
