//! The frontend reuses complete SOURCE-qualified native constructor owners.
use super::{model, third_model};
use model::{Case, Family, Recipe};

pub(super) fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for code in [5, 6] {
        for family in [Family::LegacyText, Family::ModernBinary, Family::Binary] {
            for descending in [false, true] {
                for recipe in [Recipe::Permutation, Recipe::NullShrinkGrow] {
                    cases.push(model::fitted(code, family, descending, recipe));
                }
            }
        }
        for family in [Family::LegacyBinary, Family::ModernBinary] {
            for indexes in [false, true] {
                cases.push(model::growing(code, family, indexes));
            }
        }
        for exceeds in [false, true] {
            cases.push(model::capacity(code, exceeds));
        }
        for family in [Family::LegacyText, Family::ModernBinary] {
            for descending in [false, true] {
                cases.push(third_model::case(code, family, descending));
            }
        }
    }
    cases
}
