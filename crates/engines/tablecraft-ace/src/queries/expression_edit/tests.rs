use super::{QueryAttribute, QueryFragmentExpressionEdit};
use crate::{NativeImage, QueryExpressionEditsError};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{fixtures::access::query_expression_edits as raw, support::bytes::hex};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(72),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn edit<'a>(expected: &'a str, replacement: &'a str) -> QueryFragmentExpressionEdit<'a> {
    QueryFragmentExpressionEdit {
        attribute: QueryAttribute::Column,
        order: [0, 0, 0, 1],
        expected,
        replacement,
    }
}

#[test]
fn complete_raw_growth_shrink_and_repeated_forwarding_match_without_sdk_paths() {
    let mut before = raw::source();
    for (expected, replacement) in [
        ("42", "1000"),
        ("1000", "7"),
        ("7", "1000000"),
        ("1000000", "1000000"),
    ] {
        let oracle = raw::replace(&before, replacement);
        let plan = image(&before)
            .plan_query_expression_edits(
                "Saved",
                &[edit(expected, replacement)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), oracle);
        assert_eq!(plan.native_id(), -2147483525);
        assert_eq!(plan.changed(), expected != replacement);
        before = oracle;
    }
}

#[test]
fn wrong_identity_duplicate_slots_capacity_and_cancellation_publish_nothing() {
    let before = raw::source();
    assert!(
        image(&before)
            .plan_query_expression_edits("Saved", &[edit("43", "1000")], &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image(&before)
            .plan_query_expression_edits(
                "Saved",
                &[edit("42", "1000"), edit("42", "7")],
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    assert!(
        image(&before)
            .plan_query_expression_edits(
                "Saved",
                &[edit("42", &"1".repeat(33))],
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    assert!(
        image(&before)
            .plan_query_expression_edits("Saved", &[edit("42", "7")], &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        image(&before)
            .plan_query_expression_edits(
                "Saved",
                &[edit("42", "7")],
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn present_empty_compiled_payload_refuses_while_legacy_language_stays_closed() {
    let mut before = raw::source();
    let end = 4 * 4096
        + u16::from_le_bytes(before[4 * 4096 + 28..4 * 4096 + 30].try_into().unwrap()) as usize;
    before[end - 2] |= 0x40;
    assert!(matches!(
        image(&before).plan_query_expression_edits(
            "Saved",
            &[edit("42", "7")],
            &mut budget(),
            &|| false
        ),
        Err(QueryExpressionEditsError::Source)
    ));
    let original = raw::source();
    assert!(
        image(&original)
            .plan_query_literal_expression("Saved", "42", "1000", &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
#[ignore = "private generated SDK full images; explicit separate gate only"]
fn sdk_complete_images_qualify_projection_where_multiple_pages_and_repeated_roots() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let original =
        root.join(".cache/querydef-existing-save-sdk-20261108/files/multi-page-before.accdb");
    let repeat = root.join(".cache/querydef-repeat-save-sdk-20261108/files");
    let cases = [
        (
            original.clone(),
            repeat.join("repeat-grow.accdb"),
            "Constants",
            vec![edit("42", "1000")],
            "9c8570f86bdf1687410bc5a670773549d5e1c4c5ae75f336527051ba37099a7b",
        ),
        (
            repeat.join("repeat-grow.accdb"),
            repeat.join("repeat-shrink.accdb"),
            "Constants",
            vec![edit("1000", "7")],
            "c6f06698c520fb52d3e96426f900f9586b92c1c97ad9472dc1aa02e277ff8bae",
        ),
        (
            repeat.join("repeat-shrink.accdb"),
            repeat.join("repeat-grow-again.accdb"),
            "Constants",
            vec![edit("7", "1000000")],
            "bdc88830640664cf212ffbe024cb98851835c70b938ffd205295399a7e4df0da",
        ),
        (
            repeat.join("repeat-grow-again.accdb"),
            repeat.join("repeat-noop.accdb"),
            "Constants",
            vec![edit("1000000", "1000000")],
            "bdc88830640664cf212ffbe024cb98851835c70b938ffd205295399a7e4df0da",
        ),
        (
            original.clone(),
            repeat.join("two-projections.accdb"),
            "ManyLocalColumns",
            vec![
                QueryFragmentExpressionEdit {
                    attribute: QueryAttribute::Column,
                    order: [0, 0, 0, 96],
                    expected: "95",
                    replacement: "1000",
                },
                QueryFragmentExpressionEdit {
                    attribute: QueryAttribute::Column,
                    order: [0, 0, 0, 95],
                    expected: "94",
                    replacement: "7000",
                },
            ],
            "90db6ebaa3ef3eece0b819e6a11ac5fb91bd9b2c3dd94141d11bc944780bc968",
        ),
        (
            original,
            repeat.join("projection-where.accdb"),
            "ManyLocalColumns",
            vec![
                QueryFragmentExpressionEdit {
                    attribute: QueryAttribute::Column,
                    order: [0, 0, 0, 96],
                    expected: "95",
                    replacement: "1000",
                },
                QueryFragmentExpressionEdit {
                    attribute: QueryAttribute::Where,
                    order: [0, 0, 0, 1],
                    expected: "[p].[ID] >= 2",
                    replacement: "[p].[ID] >= 1000",
                },
            ],
            "3c437023d5b6d57bc3436e04a8dccba499f8fa77f57250acb4baa3f32c6bab6b",
        ),
    ];
    for (before_path, after_path, query, edits, after_hash) in cases {
        let before = std::fs::read(before_path).unwrap();
        let after = std::fs::read(after_path).unwrap();
        assert_eq!(hex(&Sha256::digest(&after)), after_hash);
        let plan = image(&before)
            .plan_query_expression_edits(query, &edits, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), after);
    }
}

#[test]
fn foreign_owned_data_page_claim_refuses_before_any_candidate() {
    let mut before = raw::source();
    // Relationships owned-map row1: claim existing MSysQueries DATA page8.
    let map = 3 * 4096
        + u16::from_le_bytes(before[3 * 4096 + 16..3 * 4096 + 18].try_into().unwrap()) as usize;
    before[map + 6] |= 1;
    assert!(
        image(&before)
            .plan_query_expression_edits("Saved", &[edit("42", "7")], &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn distinct_high_byte_local_id_cannot_alias_query_fragment_definition() {
    let mut before = raw::source();
    // Independent native catalog slot5 is MSysRelationships. Its distinct raw
    // local ID 0x01000007 resolves to the existing MSysQueries TD 7. This changes
    // identity without inventing another page or using production format masks.
    let start = 4 * 4096
        + u16::from_le_bytes(before[4 * 4096 + 24..4 * 4096 + 26].try_into().unwrap()) as usize;
    assert_eq!(&before[start + 2..start + 6], &[5, 0, 0, 0]);
    before[start + 2..start + 6].copy_from_slice(&[7, 0, 0, 1]);

    let native = image(&before);
    let alias = crate::inspect_table_schema(
        &mut native.reader(),
        CatalogScope::new(72),
        "MSysRelationships",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(alias.definition_page(), 7);
    assert!(matches!(
        native.plan_query_expression_edits("Saved", &[edit("42", "7")], &mut budget(), &|| false),
        Err(QueryExpressionEditsError::Source)
    ));
}

#[test]
fn two_same_page_projections_match_full_raw_oracle_in_submitted_order_atomically() {
    let before = raw::two_projections();
    let untouched = before.clone();
    // Order2 is deliberately submitted before Order1, reversing source ordinal.
    let requests = [
        QueryFragmentExpressionEdit {
            attribute: QueryAttribute::Column,
            order: [0, 0, 0, 2],
            expected: "99",
            replacement: "7000",
        },
        edit("42", "1000"),
    ];
    let oracle = raw::replace_at(&raw::replace_at(&before, 4, "7000"), 3, "1000");
    let plan = image(&before)
        .plan_query_expression_edits("Saved", &requests, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), untouched);
    assert_eq!(plan.after_bytes(), oracle);
    assert_eq!(plan.edits()[0].as_request().order, [0, 0, 0, 2]);
    assert_eq!(plan.edits()[1].as_request().order, [0, 0, 0, 1]);
    assert_eq!(&plan.after_bytes()[..8 * 4096], &before[..8 * 4096]);

    let old = &before[8 * 4096..9 * 4096];
    let new = &plan.after_bytes()[8 * 4096..9 * 4096];
    assert_eq!(u16::from_le_bytes(new[12..14].try_into().unwrap()), 8);
    let mut end = 4096;
    for slot in 0..6 {
        let directory = 14 + 2 * slot;
        let word = u16::from_le_bytes(old[directory..directory + 2].try_into().unwrap());
        let start = usize::from(word);
        let fresh = u16::from_le_bytes(new[directory..directory + 2].try_into().unwrap());
        if slot == 3 || slot == 4 {
            assert_eq!(fresh, word | 0x4000);
            let target = if slot == 4 { 6 } else { 7 };
            assert_eq!(
                u32::from_le_bytes(new[start..start + 4].try_into().unwrap()),
                (8 << 8) | target
            );
            assert!(new[start + 4..end].iter().all(|byte| *byte == 0));
        } else {
            assert_eq!(fresh, word);
            assert_eq!(&new[start..end], &old[start..end]);
        }
        end = start;
    }
    // If the second physical placement refuses, no partial candidate escapes.
    let over_capacity = "1".repeat(33);
    let invalid = [
        QueryFragmentExpressionEdit {
            attribute: QueryAttribute::Column,
            order: [0, 0, 0, 2],
            expected: "99",
            replacement: "7000",
        },
        edit("42", &over_capacity),
    ];
    assert!(
        image(&before)
            .plan_query_expression_edits("Saved", &invalid, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(before, untouched);
}
