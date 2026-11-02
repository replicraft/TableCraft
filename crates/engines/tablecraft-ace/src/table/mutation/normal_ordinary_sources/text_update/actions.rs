//! Actual action Source consumers and still-closed public mutation boundaries.
use super::{SCOPE, budget, hash};
use crate::{
    EditError, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, RecoverableEditError,
    RowDelete, RowInsert, SlotScalarUpdate, WriteBlocker, inspect_table_schema,
    table::mutation::{
        fixed_records::indexed::CheckedIndexes, no_required_programs::NoRequiredTablePrograms,
        normal_actions, normal_ordinary,
    },
};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::values::Value;

fn originals() -> Vec<(Vec<u8>, &'static str, u32)> {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut result = Vec::new();
    for (file, digest, first) in [
        (
            "Encoded-V2016-before.accdb",
            "e2b5193dc4eb6c024e1e2883e894d64b4ff82c435ca441f0745e677615fc9276",
            89,
        ),
        (
            "Encoded-V2019-before.accdb",
            "636e137ee3a1d51f6525072de87963cb33a03b01a253be1d2124cf6bb4746279",
            106,
        ),
    ] {
        let bytes = fs::read(directory.join(file)).unwrap();
        assert_eq!(hash(&bytes), digest);
        for (name, definition) in [("TextLegacyLeaf", first), ("TextModernLeaf", first + 7)] {
            result.push((bytes.clone(), name, definition));
        }
    }
    result
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn reject(bytes: &[u8], table: &str) {
    let changed = image(bytes);
    assert!(normal_actions::capture(&changed, table, &mut budget(), &|| false).is_err());
    assert_eq!(changed.as_bytes(), bytes);
}

#[test]
#[ignore = "requires pinned originals; malformed derivatives grant no new profile"]
fn unknown_text_declarations_aliases_and_map_roles_refuse_full_action_source() {
    let mut controls = 0;
    for (bytes, table, definition) in originals() {
        let original = image(&bytes);
        let source = normal_actions::capture(&original, table, &mut budget(), &|| false).unwrap();
        assert_eq!(source.records.len(), 24);
        let descriptor = definition as usize * 4096 + 124;
        assert_eq!(&bytes[descriptor + 15..descriptor + 17], &[2, 0]);
        // Exact complete LCID, variant, version and declaration bytes. French
        // must not alias General, even where a separate reader recognizes it.
        for (offset, replacement) in [
            (0, vec![11]),
            (0, vec![12]),
            (11, vec![0, 0]),
            (11, vec![12, 4]),
            (13, vec![1]),
            (14, vec![2]),
            (15, vec![0]),
            (15, vec![3]),
            (15, vec![6]),
            (16, vec![1]),
            (16, vec![2]),
            (16, vec![32]),
            (23, vec![0, 0]),
            (23, vec![255, 1]),
            (23, vec![0, 2]),
        ] {
            let mut malformed = bytes.clone();
            malformed[descriptor + offset..descriptor + offset + replacement.len()]
                .copy_from_slice(&replacement);
            reject(&malformed, table);
            controls += 1;
        }
        let schema = source.table.schema();
        let physical = definition as usize * 4096 + schema.layout.index_start;
        assert_eq!(
            (
                schema.layout.physical_indexes,
                schema.layout.logical_indexes
            ),
            (3, 4)
        );
        let logical = physical + 3 * 52;
        // Twin's physical data number disagrees with its existing Asc alias.
        let mut alias = bytes.clone();
        let twin = source
            .indexes
            .indexes()
            .iter()
            .position(|index| index.name() == "Twin")
            .unwrap();
        alias[logical + twin * 28 + 8..logical + twin * 28 + 12]
            .copy_from_slice(&99_u32.to_le_bytes());
        reject(&alias, table);
        controls += 1;
        // An INDEX root cannot simultaneously be the selected DATA page.
        let mut data_overlap = bytes.clone();
        data_overlap[physical + 52 + 38..physical + 52 + 42]
            .copy_from_slice(&(definition + 5).to_le_bytes());
        reject(&data_overlap, table);
        controls += 1;
        // Distinct physical INDEX owners cannot share their actual map record.
        let mut map_alias = bytes.clone();
        let reference = bytes[physical + 34..physical + 38].to_vec();
        map_alias[physical + 52 + 34..physical + 52 + 38].copy_from_slice(&reference);
        reject(&map_alias, table);
        controls += 1;
        // The physical reserved flags and logical foreign-reference fields
        // remain part of the full raw declaration, not just a text decoder.
        let mut unknown_flags = bytes.clone();
        unknown_flags[physical + 52 + 46] |= 0x10;
        reject(&unknown_flags, table);
        controls += 1;
        let mut ignored_nulls = bytes.clone();
        ignored_nulls[physical + 52 + 46] |= 2;
        reject(&ignored_nulls, table);
        controls += 1;
        let mut foreign = bytes.clone();
        foreign[logical + 28 + 23] = 2;
        reject(&foreign, table);
        controls += 1;
        assert_eq!(original.as_bytes(), bytes);
    }
    assert_eq!(controls, 84);
}

#[test]
#[ignore = "requires pinned originals; same-image certificate and old mutation refusals"]
fn text_action_certificate_never_transfers_between_images_or_grants_public_mutation() {
    let mut contexts = 0;
    for (bytes, table, _) in originals() {
        let original = image(&bytes);
        let source = normal_actions::capture(&original, table, &mut budget(), &|| false).unwrap();
        let certificate = NoRequiredTablePrograms::capture_actions(
            &original,
            source.table.schema(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let foreign = image(&bytes);
        let foreign_schema =
            inspect_table_schema(&mut foreign.reader(), SCOPE, table, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert!(matches!(
            certificate.require_source(&foreign, &foreign_schema, &mut budget(), &|| false),
            Err(EditError::Preservation)
        ));
        assert!(matches!(
            CheckedIndexes::capture_actions(&original, table, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
        ));
        assert!(matches!(
            normal_ordinary::capture(&original, table, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::ActiveContent))
        ));
        for empty in [false, true] {
            let ordinals = if empty { Vec::new() } else { vec![0] };
            assert!(matches!(
                original.prepare_ordinary_row_deletes(table, &ordinals, &mut budget(), &|| false),
                Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
            ));
            let updates = if empty {
                Vec::new()
            } else {
                vec![SlotScalarUpdate::new(
                    4,
                    Value::Text("a".into()),
                    Value::Text("A".into()),
                )]
            };
            let group = NativeFittedIntentGroup::new(
                "KeyValue",
                NativeFittedUpdates::Variable(updates),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let error = image(&bytes)
                .plan_recoverable_ordinary_fitted_updates(
                    table,
                    vec![group],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::ActiveContent))
            ));
            let rows = if empty {
                Vec::new()
            } else {
                vec![RowInsert::new(vec![
                    Value::Int32(25),
                    Value::Text("a".into()),
                ])]
            };
            let error = image(&bytes)
                .plan_recoverable_ordinary_complete_row_inserts(table, rows, &mut budget(), &|| {
                    false
                })
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::IndexedColumn))
            ));
            let deletes = if empty {
                Vec::new()
            } else {
                vec![RowDelete::new(0, source.records[0].data.clone())]
            };
            let error = image(&bytes)
                .plan_recoverable_ordinary_row_deletes(table, deletes, &mut budget(), &|| false)
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::IndexedColumn))
            ));
        }
        assert_eq!(original.as_bytes(), bytes);
        contexts += 1;
    }
    assert_eq!(contexts, 4);
}
