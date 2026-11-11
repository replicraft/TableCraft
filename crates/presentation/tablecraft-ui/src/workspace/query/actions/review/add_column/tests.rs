//! Exact native dimensions are shown from captured declaration data.
use super::show;
use crate::{FluentArgs, Locale, Localizer};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::num::NonZeroU8;
use tablecraft_builders::table::TypedFieldAdditionDeclaration;

#[test]
fn unadorned_summary_keeps_native_text_units_and_decimal_parameters_in_both_locales() {
    for locale in [Locale::EnUs, Locale::De] {
        for declaration in [
            TypedFieldAdditionDeclaration::short_text(NonZeroU8::new(80).unwrap(), false),
            TypedFieldAdditionDeclaration::decimal(28, 4).unwrap(),
        ] {
            let localizer = Localizer::new(locale).unwrap();
            let mut harness = Harness::builder().build_ui_state(
                |ui, state: &mut (Localizer, TypedFieldAdditionDeclaration)| {
                    crate::localization::install(ui.ctx(), &state.0);
                    show(ui, "ExactValue", state.1);
                },
                (localizer, declaration),
            );
            crate::configure(&harness.ctx);
            harness.run_steps(3);
            harness.get_by_role_and_label(Role::Label, "ExactValue");
            if declaration.short_text_units().is_some() {
                harness.get_by_role_and_label(Role::Label, "80");
                let label = harness.state().0.text("table-draft-short-text");
                harness.get_by_role_and_label(Role::Label, &label);
            } else {
                let label = harness.state().0.text("table-draft-number-decimal");
                harness.get_by_role_and_label(Role::Label, &label);
                let mut arguments = FluentArgs::new();
                arguments.set("precision", "28");
                arguments.set("scale", "4");
                let label = harness
                    .state()
                    .0
                    .format("design-precision-value", &arguments);
                harness.get_by_role_and_label(Role::Label, &label);
            }
        }
    }
}
