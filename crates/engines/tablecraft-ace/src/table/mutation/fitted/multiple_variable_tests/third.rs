//! SOURCE-only gate over the same pure additive third-variable owner.
use super::{model::Family, qualify_case, third_model};

#[test]
fn qualify_three_variable_sources_and_complete_expected_before_preview() {
    for code in [5, 6] {
        for family in [Family::LegacyText, Family::ModernBinary] {
            for descending in [false, true] {
                qualify_case(third_model::case(code, family, descending));
            }
        }
    }
}
