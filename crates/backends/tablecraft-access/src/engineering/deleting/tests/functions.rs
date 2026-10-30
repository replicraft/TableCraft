//! Deterministic predicates over the actual independent DELETE image owner.
use super::{budget, capture, expected, qualify, source};
use crate::engineering::{DeleteMode, sql_delete};
use tablecraft_sql::{ParseLimits, parse_query};

const RECIPES: [(&str, &[usize]); 4] = [
    ("Len(Trim('  x  '))=0", &[]),
    ("Abs(Id-2)=0", &[1]),
    ("CLng(Round(Id/2.0,0))<>1", &[0, 2]),
    ("Nz(Id,0)=Id", &[0, 1, 2]),
];

#[test]
fn qualify_function_delete_sources_and_complete_expected_images_before_function_api() {
    for code in [5, 6] {
        let before = source(code);
        qualify(&before, 3);
        for (_, selected) in RECIPES {
            qualify(&expected(&before, selected), 3 - selected.len());
        }
    }
}

#[test]
fn deterministic_delete_predicates_match_the_original_complete_images() {
    for code in [5, 6] {
        for (predicate, selected) in RECIPES {
            let before = source(code);
            let after = expected(&before, selected);
            let parsed = parse_query(
                &format!("DELETE FROM Items WHERE {predicate}"),
                ParseLimits::default(),
            )
            .unwrap();
            let plan = sql_delete::preview_delete(
                capture(&before),
                parsed.query(),
                DeleteMode::Restrict,
                &mut budget(),
                None,
                &|| false,
            )
            .unwrap();
            assert_eq!(plan.matched_rows(), selected.len());
            let copy = plan.into_backend();
            assert_eq!(copy.before_bytes(), before);
            assert_eq!(copy.candidate_bytes(), after);
        }
    }
}
