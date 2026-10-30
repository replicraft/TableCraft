//! Function spellings over the original independent six-image tuple corpus.
use super::{
    budget, image,
    model::{Recipe, case},
    qualify,
};
use crate::engineering::{ScalarUpdateMode, UpdateMode, sql};
use tablecraft_sql::{ParseLimits, parse_query};

const RECIPES: [(Recipe, &str); 3] = [
    (
        Recipe::Clear,
        "UPDATE Items SET [Group]=IIf(Id=1,NULL,[Group]), Score=IIf(Id=1,NULL,Score)",
    ),
    (
        Recipe::Exchange,
        "UPDATE Items SET [Group]=Abs(Score), Score=Abs([Group])",
    ),
    (
        Recipe::Introduce,
        "UPDATE Items SET [Group]=CLng(20.0), Score=CLng(Round(200.0,0)) WHERE Id=3",
    ),
];

#[test]
fn qualify_function_tuple_sources_and_complete_expected_images_before_function_api() {
    for code in [5, 6] {
        for (recipe, _) in RECIPES {
            let example = case(code, recipe);
            qualify(&example.before, &example.original);
            qualify(&example.expected, &example.final_values);
        }
    }
}

#[test]
fn deterministic_functions_finish_once_into_the_original_frozen_tuple_images() {
    for code in [5, 6] {
        for (recipe, text) in RECIPES {
            let example = case(code, recipe);
            let parsed = parse_query(text, ParseLimits::default()).unwrap();
            let plan = sql::preview_batch_update(
                image(&example.before),
                parsed.query(),
                ScalarUpdateMode::Fixed(UpdateMode::Fitted),
                &mut budget(),
                None,
                &|| false,
            )
            .unwrap();
            let copy = plan.into_backend();
            assert_eq!(copy.candidate_bytes(), example.expected);
            qualify(copy.candidate_bytes(), &example.final_values);
        }
    }
}
