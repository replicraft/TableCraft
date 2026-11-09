//! Native candidates versus independently authored complete raw/reader hypotheses.
use crate::{
    CreateDatabaseError, DatabaseNameAutoCorrect, FieldDropError, InspectError, NativeImage,
    PropertyGroupKind, inspect_database_properties, inspect_table_properties, inspect_table_schema,
};
use sha2::{Digest, Sha256};
use std::{cell::Cell, fs, io::Cursor, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_before_change as program_raw,
        canonical_created_number::{Source, mixed::after_off::deleted_field as raw, repeated},
    },
    support::bytes::hex,
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1203);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn two_candidates_match_appended_group_removed_pins_not_readable_orphan_controls() {
    for (case, selected, pin, surviving) in [
        (
            raw::SourceCase::NumberHistory,
            "Score",
            "6c08831dfe4d305064837d93a8d1942a1a87b691ca80e4834a130c202c72923b",
            vec![("ID", 0), ("Score2", 2)],
        ),
        (
            raw::SourceCase::MixedTextHistory,
            "Label",
            "d9432f5edadc0bfcd60551f747be5e6ac630585f5cfa78fffdced6f7e875b935",
            vec![("ID", 0), ("AfterLabel", 2), ("OtherLabel", 3)],
        ),
    ] {
        let before = raw::source(case);
        let expected =
            raw::remove_nonprimary(&before, selected, raw::Variant::AppendedGroupRemoved);
        let orphan = raw::remove_nonprimary(&before, selected, raw::Variant::DeclarationOnly);
        let original = inspect_table_schema(
            &mut Cursor::new(&before),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let captured = image(&before);
        let native_id = captured
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == original.object_id())
            .unwrap()
            .native_id();
        let observed = captured
            .inspect_saved_field_drop_source("Table1", selected, &mut budget(), &|| false)
            .unwrap();
        let candidate = captured
            .prepare_saved_field_drop("Table1", selected, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.before_bytes(), before);
        assert_eq!(candidate.after_bytes(), expected);
        assert_ne!(candidate.after_bytes(), orphan);
        assert_eq!(hex(&Sha256::digest(candidate.after_bytes())), pin);
        assert_eq!(candidate.table(), "Table1");
        assert_eq!(candidate.column(), selected);
        assert_eq!(candidate.identity().object_id(), original.object_id());
        assert_eq!(candidate.identity().native_table_id(), native_id);
        assert_eq!(candidate.identity().definition_page(), 7);
        assert_eq!(candidate.identity().column_number(), 1);
        assert_eq!(candidate.identity().column_id(), 1);
        assert_eq!(candidate.identity(), observed);
        let actual = inspect_table_schema(
            &mut Cursor::new(candidate.after_bytes()),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            actual
                .columns()
                .iter()
                .map(|field| (field.name(), field.number()))
                .collect::<Vec<_>>(),
            surviving
        );
        assert_eq!(
            actual.physical_layout().max_columns,
            original.physical_layout().max_columns
        );
        assert_eq!(
            actual.physical_layout().variable_slots,
            original.physical_layout().variable_slots
        );
        assert_eq!(
            actual.physical_layout().fixed_end,
            original.physical_layout().fixed_end
        );
        assert_eq!(actual.declared_rows(), 0);
        for field in actual.columns() {
            let previous = original
                .columns()
                .iter()
                .find(|old| old.number() == field.number())
                .unwrap();
            assert!(
                field.physical_field() == previous.physical_field(),
                "surviving storage declarations stay exact"
            );
            if field.name() == "OtherLabel" {
                assert_eq!(
                    field.physical_field().variable,
                    1,
                    "removing variable0 does not compact surviving variable1"
                );
            }
        }
        for page in [0, 1, 2, 3, 5, 6, 8, 9] {
            assert_eq!(
                &candidate.after_bytes()[page * PAGE..(page + 1) * PAGE],
                &before[page * PAGE..(page + 1) * PAGE]
            );
        }
    }
}

#[test]
fn live_group_removal_preserves_other_native_properties_and_explicit_database_off() {
    for (case, selected) in [
        (raw::SourceCase::NumberHistory, "Score"),
        (raw::SourceCase::MixedTextHistory, "Label"),
    ] {
        let before = raw::source(case);
        let original = inspect_table_properties(
            &mut Cursor::new(&before),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let candidate = image(&before)
            .prepare_saved_field_drop("Table1", selected, &mut budget(), &|| false)
            .unwrap();
        let actual = inspect_table_properties(
            &mut Cursor::new(candidate.after_bytes()),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(actual.group(PropertyGroupKind::Column, selected).is_none());
        for group in original.groups().iter().filter(|group| {
            !(group.kind() == PropertyGroupKind::Column && group.name() == selected)
        }) {
            let kept = actual.group(group.kind(), group.name()).unwrap();
            assert_eq!(kept.properties().len(), group.properties().len());
            for (new, old) in kept.properties().iter().zip(group.properties()) {
                assert_eq!(new.name(), old.name());
                assert_eq!(new.native_type(), old.native_type());
                assert_eq!(new.flags(), old.flags());
                assert_eq!(new.raw_value(), old.raw_value());
            }
        }
        let database_before =
            inspect_database_properties(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                .unwrap()
                .unwrap();
        let database_after = inspect_database_properties(
            &mut Cursor::new(candidate.after_bytes()),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(database_after.object_id(), database_before.object_id());
        assert_eq!(database_after.raw_bytes(), database_before.raw_bytes());
        assert_eq!(
            database_after
                .name_auto_correct(&mut budget(), &|| false)
                .unwrap(),
            DatabaseNameAutoCorrect::ExplicitOff
        );
    }
}

#[test]
fn primary_missing_or_unused_dictionary_key_targets_never_publish_a_candidate() {
    let before = raw::source(raw::SourceCase::MixedTextHistory);
    for column in ["ID", "Missing"] {
        assert!(matches!(
            image(&before).prepare_saved_field_drop("Table1", column, &mut budget(), &|| false),
            Err(FieldDropError::Source)
        ));
    }
    // AfterLabel alone uses DefaultValue; retained dictionary keys may not be
    // silently pruned or newly admitted as unused when that group is removed.
    assert!(matches!(
        image(&before).prepare_saved_field_drop("Table1", "AfterLabel", &mut budget(), &|| false),
        Err(FieldDropError::Declaration(
            CreateDatabaseError::Declaration
        ))
    ));
    assert!(matches!(
        image(&before).inspect_saved_field_drop_source(
            "Table1",
            "AfterLabel",
            &mut budget(),
            &|| false
        ),
        Err(FieldDropError::Declaration(
            CreateDatabaseError::Declaration
        ))
    ));
}

#[test]
fn missing_off_and_unknown_original_complement_do_not_gain_drop_authority() {
    let missing = repeated::source(Source::IdOnly);
    assert!(matches!(
        image(&missing).prepare_saved_field_drop("Table1", "Score", &mut budget(), &|| false),
        Err(FieldDropError::SourceProof(_))
    ));
    let before = raw::source(raw::SourceCase::NumberHistory);
    for offset in [3 * PAGE + 512, 7 * PAGE + 3000, 11 * PAGE - 1] {
        let mut changed = before.clone();
        changed[offset] ^= 1;
        assert!(matches!(
            image(&changed).prepare_saved_field_drop("Table1", "Score", &mut budget(), &|| false),
            Err(FieldDropError::SourceProof(_))
        ));
    }
}

#[test]
fn cancelled_and_exhausted_candidate_work_preserve_original_bytes() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    assert!(matches!(
        image(&before).prepare_saved_field_drop("Table1", "Score", &mut budget(), &|| true),
        Err(FieldDropError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&before).prepare_saved_field_drop(
            "Table1",
            "Score",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(FieldDropError::Inspect(InspectError::Limit(_)))
    ));
    assert_eq!(before, raw::source(raw::SourceCase::NumberHistory));
}

#[test]
fn indexed_selected_field_relationships_and_stored_program_sources_refuse_before_output() {
    let mut indexed = raw::source(raw::SourceCase::NumberHistory);
    // Independent literal source framing: prefix75 + three25-byte entries +
    // ID/Score/Score2 name records(6+12+14), then first physical key at +4.
    // Change only the stored key component to the actual selected native1.
    let key = 7 * PAGE + 186;
    assert_eq!(&indexed[key..key + 2], &[0, 0]);
    indexed[key..key + 2].copy_from_slice(&1_u16.to_le_bytes());
    let indexes = crate::inspect_table_indexes(
        &mut Cursor::new(&indexed),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(indexes.indexes()[0].columns()[0].number(), 1);
    assert!(matches!(
        image(&indexed).prepare_saved_field_drop("Table1", "Score", &mut budget(), &|| false),
        Err(FieldDropError::SourceProof(_))
    ));

    // Reuse the tracked reader fixture already consumed by relationships.rs.
    // This is another real source outside the closed settled language, not a
    // claim that general relationship-dependent DROP has been implemented.
    let relationship_bytes = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let relationships = crate::inspect_relationships(
        &mut Cursor::new(&relationship_bytes),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(relationships.len(), 1);
    assert_eq!(
        relationships[0].declaration().parent_table().as_str(),
        "TAG_GRP_TB"
    );
    assert!(matches!(
        image(&relationship_bytes).prepare_saved_field_drop(
            "TAG_GRP_TB",
            "TAG_GRP_ID",
            &mut budget(),
            &|| false
        ),
        Err(FieldDropError::SourceProof(_))
    ));

    let program = program_raw::source(6, program_raw::Recipe::Sequential);
    let program_image = image(&program.before);
    let schema = inspect_table_schema(
        &mut Cursor::new(&program.before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let extra = program_image
        .inspect_table_extra(schema.object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(extra.raw(), Some(program.extra.as_slice()));
    assert!(!program.extra.is_empty());
    assert!(matches!(
        program_image.prepare_saved_field_drop("Items", "Score", &mut budget(), &|| false),
        Err(FieldDropError::SourceProof(_))
    ));
}

#[test]
fn measured_complete_candidate_pool_and_middle_terminal_cancellation_never_publish_prefixes() {
    let before = raw::source(raw::SourceCase::MixedTextHistory);
    let visits = Cell::new(0_usize);
    let mut work = budget();
    let complete = image(&before)
        .prepare_saved_field_drop("Table1", "Label", &mut work, &|| {
            visits.set(visits.get() + 1);
            false
        })
        .unwrap();
    let total = visits.get();
    let spent = 5_000_000 - work.remaining(Resource::Steps);
    assert!(total > 2 && spent > 1);
    assert!(work.remaining(Resource::Bytes) < 512 * 1024 * 1024);
    let exact = image(&before)
        .prepare_saved_field_drop(
            "Table1",
            "Label",
            &mut ResourceBudget::new(512 * 1024 * 1024, 100_000, spent),
            &|| false,
        )
        .unwrap();
    assert_eq!(exact.after_bytes(), complete.after_bytes());
    assert!(matches!(
        image(&before).prepare_saved_field_drop(
            "Table1",
            "Label",
            &mut ResourceBudget::new(512 * 1024 * 1024, 100_000, spent - 1),
            &|| false
        ),
        Err(FieldDropError::Inspect(InspectError::Limit(_)))
    ));
    for stop in [1, total / 2, total] {
        visits.set(0);
        let result =
            image(&before).prepare_saved_field_drop("Table1", "Label", &mut budget(), &|| {
                visits.set(visits.get() + 1);
                visits.get() >= stop
            });
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("cancellation may not publish a partial candidate"),
        };
        assert_eq!(error.to_string(), InspectError::Cancelled.to_string());
        assert!(
            !matches!(error, FieldDropError::Source | FieldDropError::Preservation),
            "control failure must not become source refusal"
        );
    }
    assert_eq!(before, raw::source(raw::SourceCase::MixedTextHistory));
}
