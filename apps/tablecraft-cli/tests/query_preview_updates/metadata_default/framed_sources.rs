//! Read-only qualification before switching any command mutation fixture.
use super::{capacity, framed, model, preservation, qualify};
use model::{Case, Family, Recipe};

fn pair(original: Case) {
    let derived = framed::framed(original);
    qualify(&derived.before, &derived.rows, derived.family);
    qualify(&derived.expected, &derived.final_rows, derived.family);
    preservation(&derived);
}

#[test]
fn qualify_cli_framed_fitting_derivatives_before_command_mutation() {
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for recipe in [Recipe::Permutation, Recipe::Shorter, Recipe::Nulls] {
                    pair(model::case(code, family, descending, recipe, false));
                }
            }
        }
    }
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        pair(model::case(6, family, true, Recipe::Permutation, true));
    }
    for family in [Family::Legacy, Family::Modern] {
        for descending in [false, true] {
            pair(capacity::case(family, descending));
        }
    }
}
