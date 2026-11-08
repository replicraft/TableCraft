use super::*;
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::saved_queries::fitting_select::storage_pair;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn constant_expression_candidate_matches_complete_independent_after() {
    let [before, expected] = storage_pair();
    let plan = capture(&before)
        .plan_query_literal_expression("Saved", "42", "43", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(plan.query(), "Saved");
    assert_eq!(plan.native_id(), -2147483525);
    assert_eq!(plan.expected_expression(), "42");
    assert_eq!(plan.replacement_expression(), "43");
    assert!(plan.changed());
}

#[test]
fn exact_noop_retains_all_source_qualification() {
    let [before, _] = storage_pair();
    let plan = capture(&before)
        .plan_query_literal_expression("Saved", "42", "42", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), before);
    assert!(!plan.changed());

    let mut unsupported = before.clone();
    // The source's selected catalog row is the last of nine direct records.
    // Present-empty LvProp is opaque content, not absent cache/property storage.
    let end = 4 * 4096
        + usize::from(u16::from_le_bytes(
            unsupported[4 * 4096 + 28..4 * 4096 + 30]
                .try_into()
                .unwrap(),
        ));
    unsupported[end - 2] |= 0x40;
    assert!(matches!(
        capture(&unsupported).plan_query_literal_expression(
            "Saved",
            "42",
            "42",
            &mut budget(),
            &|| false,
        ),
        Err(QueryExpressionError::Source)
    ));
}

#[test]
fn invalid_literals_mismatched_original_and_different_lengths_refuse() {
    let [before, _] = storage_pair();
    for literal in ["", "04", "-1", "1+2", "NeverRunMe()", "2147483648"] {
        assert!(matches!(
            capture(&before).plan_query_literal_expression(
                "Saved",
                "42",
                literal,
                &mut budget(),
                &|| false,
            ),
            Err(QueryExpressionError::Literal)
        ));
    }
    assert!(matches!(
        capture(&before).plan_query_literal_expression(
            "Saved",
            "42",
            "100",
            &mut budget(),
            &|| false,
        ),
        Err(QueryExpressionError::Capacity)
    ));
    assert!(matches!(
        capture(&before)
            .plan_query_literal_expression("Saved", "41", "43", &mut budget(), &|| false,),
        Err(QueryExpressionError::Definition)
    ));
}

#[test]
fn another_tables_data_claim_blocks_the_selected_query_page() {
    let [mut before, _] = storage_pair();
    // MSysRelationships' originally empty owned map is map-container slot 1.
    // Claim query DATA8 while retaining its original owner and every row byte.
    let start = 3 * 4096
        + usize::from(u16::from_le_bytes(
            before[3 * 4096 + 16..3 * 4096 + 18].try_into().unwrap(),
        ));
    before[start + 6] |= 1;
    assert!(
        capture(&before)
            .plan_query_literal_expression("Saved", "42", "43", &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn cancelled_and_exhausted_planning_publish_no_candidate() {
    let [before, _] = storage_pair();
    assert!(matches!(
        capture(&before)
            .plan_query_literal_expression("Saved", "42", "43", &mut budget(), &|| true,),
        Err(QueryExpressionError::Inspect(InspectError::Cancelled))
    ));
    assert!(
        capture(&before)
            .plan_query_literal_expression(
                "Saved",
                "42",
                "43",
                &mut ResourceBudget::new(0, 0, 0),
                &|| false,
            )
            .is_err()
    );
}

#[test]
#[ignore = "requires explicit original Jackcess Constants BEFORE/AFTER fixture paths"]
fn retained_sdk_constant_expression_matches_complete_independent_after() {
    use sha2::{Digest, Sha256};

    let before = std::fs::read(std::env::var_os("TABLECRAFT_QUERYDEF_BEFORE").unwrap()).unwrap();
    let expected = std::fs::read(std::env::var_os("TABLECRAFT_QUERYDEF_AFTER").unwrap()).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&before)),
        "83f9d69408948d64eab4dbeafcca468116b5624bdf6b9510569640497b487779"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&expected)),
        "6f4269175635cccea5d61fae371899d1efa06a1f79a78172efa75539792f6693"
    );
    let plan = capture(&before)
        .plan_query_literal_expression("Constants", "42", "43", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(plan.native_id(), -2147476648);
}
