//! Independent complete-image QueryDef creation and original-source refusals.
use super::{QueryCreationDefinition, QueryCreationError};
use crate::{InspectError, NativeImage, QueryAttribute, read_saved_query_definition};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::LegacyDateTime};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::saved_queries::creation as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn at() -> LegacyDateTime {
    LegacyDateTime::from_ole_days(45000.5).unwrap()
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(91),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn retained_image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(91),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn definition(name: &str, alias: &str, value: i32) -> QueryCreationDefinition {
    QueryCreationDefinition::capture(name, alias, value, at(), &mut budget(), &|| false).unwrap()
}

#[test]
fn new_queries_match_independent_complete_images_and_preserve_original_saved_definition() {
    for (name, alias, value) in [
        ("New query", "Answer", 43),
        ("Résumé 新🦀", "Total value", -7),
        ("Minimum", "Negative", i32::MIN),
        ("Maximum", "Positive", i32::MAX),
    ] {
        for (retained, capture) in [
            (false, image as fn(&[u8]) -> NativeImage),
            (true, retained_image as fn(&[u8]) -> NativeImage),
        ] {
            let expected = raw::source(name, alias, value, at());
            let candidate = capture(&expected.before)
                .prepare_query_creation(definition(name, alias, value), &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("{name}/{value}/retained={retained}: {error}"));
            assert_eq!(candidate.before_bytes(), expected.before);
            assert_eq!(candidate.candidate_bytes(), expected.expected);
            assert_eq!(candidate.native_query_id(), expected.native_query_id);

            let mut bytes = Cursor::new(candidate.candidate_bytes());
            let new = read_saved_query_definition(
                &mut bytes,
                CatalogScope::new(92),
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(new.origin().native_id(), expected.native_query_id);
            assert_eq!(new.rows().len(), 5);
            for (row, attribute) in new.rows().iter().zip([
                QueryAttribute::Start,
                QueryAttribute::Type,
                QueryAttribute::Flags,
                QueryAttribute::Column,
                QueryAttribute::End,
            ]) {
                assert_eq!(row.attribute(), Some(attribute));
                assert_eq!(row.native_object_id(), expected.native_query_id);
                assert_eq!(row.order(), Some([0, 0, 0, 1].as_slice()));
                assert_eq!(row.name2(), None);
                assert_eq!(row.extra(), None);
            }
            assert_eq!(new.rows()[1].flag(), Some(1));
            assert_eq!(new.rows()[2].flag(), Some(0));
            assert_eq!(new.rows()[3].name1(), Some(alias));
            assert_eq!(new.rows()[3].expression(), Some(value.to_string().as_str()));
            assert_eq!(new.rows()[3].flag(), None);
            let old = read_saved_query_definition(
                &mut bytes,
                CatalogScope::new(92),
                "Saved",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(old.rows().len(), 5);
            assert_eq!(old.rows()[3].expression(), Some("42"));
            assert_eq!(old.rows()[3].name1(), Some("Answer"));
        }
    }
}

#[test]
fn repeated_creation_uses_new_native_identity_and_preserves_both_prior_definitions() {
    let first = raw::source("First", "One", 43, at());
    let candidate = image(&first.before)
        .prepare_query_creation(definition("First", "One", 43), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), first.expected);
    let expected_id = first.native_query_id + 1;
    let expected = raw::after(&first.expected, expected_id, "Second", "Two", -5, at());
    let second = image(candidate.candidate_bytes())
        .prepare_query_creation(definition("Second", "Two", -5), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(second.native_query_id(), expected_id);
    assert_eq!(second.candidate_bytes(), expected);
}

#[test]
fn existing_negative_container_identity_participates_in_actual_query_allocation() {
    let before = raw::with_databases_identity(i32::MIN + 200);
    let expected_id = i32::MIN + 201;
    let expected = raw::after(&before, expected_id, "New", "Answer", 43, at());
    let candidate = image(&before)
        .prepare_query_creation(definition("New", "Answer", 43), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.native_query_id(), expected_id);
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.candidate_bytes(), expected);
}

#[test]
fn final_negative_identity_is_created_and_fresh_next_creation_refuses() {
    let before = raw::with_query_identity(-2);
    let expected = raw::after(&before, -1, "Last", "Answer", 43, at());
    let candidate = image(&before)
        .prepare_query_creation(definition("Last", "Answer", 43), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.native_query_id(), -1);
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.candidate_bytes(), expected);
    let created = read_saved_query_definition(
        &mut Cursor::new(candidate.candidate_bytes()),
        CatalogScope::new(94),
        "Last",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(created.origin().native_id(), -1);
    assert_eq!(created.rows().len(), 5);
    assert!(
        created
            .rows()
            .iter()
            .all(|row| row.native_object_id() == -1)
    );
    assert!(matches!(
        image(candidate.candidate_bytes()).prepare_query_creation(
            definition("After last", "Answer", 44),
            &mut budget(),
            &|| false,
        ),
        Err(QueryCreationError::Source)
    ));
}

#[test]
fn destination_collisions_orphan_owners_and_exhausted_id_space_refuse_at_real_boundaries() {
    let source = raw::source("New", "Answer", 43, at());
    for name in ["Saved", "sAvEd", "MSysObjects", "msysqueries"] {
        assert!(matches!(
            image(&source.before).prepare_query_creation(
                definition(name, "Answer", 43),
                &mut budget(),
                &|| false
            ),
            Err(QueryCreationError::Definition)
        ));
    }
    for before in [
        raw::orphaned_fragment(),
        raw::with_query_identity(-1),
        raw::overlapping_catalog_dates(),
    ] {
        assert!(matches!(
            image(&before).prepare_query_creation(
                definition("New", "Answer", 43),
                &mut budget(),
                &|| false
            ),
            Err(QueryCreationError::Source)
        ));
    }
    assert!(
        NativeImage::capture(
            &mut Cursor::new(raw::with_query_identity(7)),
            CatalogScope::new(93),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn invalid_names_cancellation_and_resource_exhaustion_do_not_yield_candidates() {
    for (name, alias) in [
        ("", "Answer"),
        ("New", ""),
        ("Bad.Name", "Answer"),
        ("New", "Bad[Alias]"),
    ] {
        assert!(matches!(
            QueryCreationDefinition::capture(name, alias, 43, at(), &mut budget(), &|| false),
            Err(QueryCreationError::Definition)
        ));
    }
    assert!(matches!(
        QueryCreationDefinition::capture("New", "Answer", 43, at(), &mut budget(), &|| true),
        Err(QueryCreationError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        QueryCreationDefinition::capture(
            "New",
            "Answer",
            43,
            at(),
            &mut ResourceBudget::new(0, 1_000_000, 500_000_000),
            &|| false
        ),
        Err(QueryCreationError::Inspect(InspectError::Limit(_)))
    ));
    let source = raw::source("New", "Answer", 43, at());
    assert!(matches!(
        image(&source.before).prepare_query_creation(
            definition("New", "Answer", 43),
            &mut budget(),
            &|| true
        ),
        Err(QueryCreationError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&source.before).prepare_query_creation(
            definition("New", "Answer", 43),
            &mut ResourceBudget::new(0, 1_000_000, 500_000_000),
            &|| false
        ),
        Err(QueryCreationError::Inspect(InspectError::Limit(_)))
    ));
}

#[test]
fn leading_bom_short_text_names_match_literal_escape_bytes_and_preserve_plain_memo() {
    let name = "\u{feff}Created";
    let alias = "\u{feff}Heading";
    let source = raw::source(name, alias, -7, at());
    for capture in [
        image as fn(&[u8]) -> NativeImage,
        retained_image as fn(&[u8]) -> NativeImage,
    ] {
        let candidate = capture(&source.before)
            .prepare_query_creation(definition(name, alias, -7), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.before_bytes(), source.before);
        assert_eq!(candidate.candidate_bytes(), source.expected);
        assert_eq!(candidate.native_query_id(), source.native_query_id);
        let actual = candidate.candidate_bytes();
        // Independent literal offsets identify the newly appended catalog row
        // and the Column fragment; no production layout/encoder supplies them.
        let row_start = |page: usize, slot: usize| {
            let at = page * 4096 + 14 + 2 * slot;
            page * 4096 + usize::from(u16::from_le_bytes(actual[at..at + 2].try_into().unwrap()))
        };
        let catalog = row_start(4, 9);
        assert_eq!(
            &actual[catalog + 32..catalog + 37],
            &[0xff, 0xfe, 0, 0xff, 0xfe]
        );
        let column = row_start(8, 8);
        assert_eq!(
            &actual[column + 17..column + 22],
            &[0xff, 0xfe, 0, 0xff, 0xfe]
        );
        let expression = column + 17 + 3 + 2 * 8;
        assert_eq!(
            &actual[expression..expression + 16],
            &[4, 0, 0, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, b'-', 0, b'7', 0]
        );
        let reopened = read_saved_query_definition(
            &mut Cursor::new(actual),
            CatalogScope::new(95),
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(reopened.object().name().as_str(), name);
        assert_eq!(reopened.rows()[3].name1(), Some(alias));
        assert_eq!(reopened.rows()[3].expression(), Some("-7"));
    }
}
