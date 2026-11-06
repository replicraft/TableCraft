//! Visible unavailable command groups preserve layout without dispatching actions.
use super::{CommandSize, Icon, command, command_menu, group};
use crate::Localizer;
use egui::Ui;

pub(in crate::workspace) fn unavailable(
    ui: &mut Ui,
    t: &Localizer,
    icon: Icon,
    key: &str,
    size: CommandSize,
) {
    let menu = matches!(
        key,
        "ribbon-more"
            | "ribbon-create-data-macros"
            | "ribbon-rename-delete-macro"
            | "ribbon-application-parts"
            | "ribbon-sharepoint-list"
            | "ribbon-navigation-form"
            | "ribbon-more-forms"
            | "ribbon-new-data-source"
            | "ribbon-export-more"
            | "ribbon-add-ins"
            | "ribbon-validation"
            | "ribbon-font-color"
            | "ribbon-text-highlight"
            | "ribbon-fill-color"
            | "ribbon-borders"
            | "ribbon-alternate-rows"
            | "ribbon-text-direction"
    );
    if menu {
        command_menu(ui, icon, &t.text(key), false, size);
    } else {
        command(ui, icon, &t.text(key), false, size);
    }
}

pub(in crate::workspace) fn home(ui: &mut Ui, t: &Localizer) {
    super::group_with_launcher(ui, &t.text("ribbon-text-formatting"), true, |ui| {
        ui.horizontal(|ui| {
            ui.add_enabled_ui(false, |ui| {
                let response = egui::ComboBox::from_id_salt("unavailable_font_family")
                    .width(120.0)
                    .selected_text("Selawik")
                    .show_ui(ui, |ui| {
                        let _ = ui.selectable_label(true, "Selawik");
                    })
                    .response
                    .on_disabled_hover_text(t.text("ribbon-unavailable"));
                describe_combo(ui, &response, &t.text("ribbon-font"), "Selawik", t);
            });
            ui.add_enabled_ui(false, |ui| {
                let response = egui::ComboBox::from_id_salt("unavailable_font_size")
                    .width(42.0)
                    .selected_text("11")
                    .show_ui(ui, |ui| {
                        let _ = ui.selectable_label(true, "11");
                    })
                    .response
                    .on_disabled_hover_text(t.text("ribbon-unavailable"));
                describe_combo(ui, &response, &t.text("ribbon-font-size"), "11", t);
            });
            ui.separator();
            unavailable(
                ui,
                t,
                Icon::Bullets,
                "ribbon-bullets",
                CommandSize::IconOnly,
            );
            unavailable(
                ui,
                t,
                Icon::Numbering,
                "ribbon-numbering",
                CommandSize::IconOnly,
            );
            ui.separator();
            unavailable(
                ui,
                t,
                Icon::DecreaseIndent,
                "ribbon-decrease-indent",
                CommandSize::IconOnly,
            );
            unavailable(
                ui,
                t,
                Icon::IncreaseIndent,
                "ribbon-increase-indent",
                CommandSize::IconOnly,
            );
            ui.separator();
            unavailable(
                ui,
                t,
                Icon::Paragraph,
                "ribbon-text-direction",
                CommandSize::IconOnly,
            );
        });
        ui.horizontal(|ui| {
            for (icon, key) in [
                (Icon::Bold, "ribbon-bold"),
                (Icon::Italic, "ribbon-italic"),
                (Icon::Underline, "ribbon-underline"),
                (Icon::TextColor, "ribbon-font-color"),
                (Icon::TextHighlight, "ribbon-text-highlight"),
                (Icon::Fill, "ribbon-fill-color"),
                (Icon::AlignLeft, "ribbon-align-left"),
                (Icon::AlignCenter, "ribbon-align-center"),
                (Icon::AlignRight, "ribbon-align-right"),
                (Icon::Borders, "ribbon-borders"),
                (Icon::AlternateRows, "ribbon-alternate-rows"),
            ] {
                if matches!(
                    key,
                    "ribbon-font-color" | "ribbon-align-left" | "ribbon-borders"
                ) {
                    ui.separator();
                }
                unavailable(ui, t, icon, key, CommandSize::IconOnly);
            }
        });
    });
}

pub(in crate::workspace) fn design(ui: &mut Ui, t: &Localizer) {
    group(ui, &t.text("ribbon-tools"), |ui| {
        ui.horizontal(|ui| {
            unavailable(ui, t, Icon::Key, "ribbon-primary-key", CommandSize::Large);
            unavailable(ui, t, Icon::Builder, "ribbon-builder", CommandSize::Large);
            unavailable(
                ui,
                t,
                Icon::Validation,
                "ribbon-test-validation-rules",
                CommandSize::Large,
            );
            ui.vertical(|ui| {
                unavailable(
                    ui,
                    t,
                    Icon::InsertRows,
                    "ribbon-insert-rows",
                    CommandSize::Small,
                );
                unavailable(
                    ui,
                    t,
                    Icon::DeleteRows,
                    "ribbon-delete-rows",
                    CommandSize::Small,
                );
                unavailable(
                    ui,
                    t,
                    Icon::Lookups,
                    "ribbon-modify-lookups",
                    CommandSize::Small,
                );
            });
        });
    });
    group(ui, &t.text("ribbon-show-hide"), |ui| {
        ui.horizontal(|ui| {
            unavailable(
                ui,
                t,
                Icon::Properties,
                "ribbon-property-sheet",
                CommandSize::Large,
            );
            unavailable(ui, t, Icon::Indexes, "ribbon-indexes", CommandSize::Large);
        });
    });
    group(ui, &t.text("ribbon-field-record-table-events"), |ui| {
        ui.horizontal(|ui| {
            unavailable(
                ui,
                t,
                Icon::MacroCreate,
                "ribbon-create-data-macros",
                CommandSize::Large,
            );
            unavailable(
                ui,
                t,
                Icon::MacroRename,
                "ribbon-rename-delete-macro",
                CommandSize::Large,
            );
        });
    });
    group(ui, &t.text("ribbon-relationships"), |ui| {
        ui.horizontal(|ui| {
            unavailable(
                ui,
                t,
                Icon::Relationships,
                "ribbon-relationships",
                CommandSize::Large,
            );
            unavailable(
                ui,
                t,
                Icon::Dependencies,
                "ribbon-object-dependencies",
                CommandSize::Large,
            );
        });
    });
}

/// Additional tabs expose their unavailable command groups without services.
/// No retained Access capture currently qualifies these tab compositions.
pub(in crate::workspace) fn tab(ui: &mut Ui, t: &Localizer, tab: super::Tab) -> bool {
    use super::Tab;

    let groups: &[(&str, &[(Icon, &str)])] = match tab {
        Tab::Create => &[
            (
                "ribbon-templates",
                &[(Icon::ApplicationParts, "ribbon-application-parts")],
            ),
            (
                "ribbon-tables",
                &[
                    (Icon::Table, "ribbon-table"),
                    (Icon::TableEdit, "ribbon-table-design"),
                    (Icon::SharePointList, "ribbon-sharepoint-list"),
                ],
            ),
            (
                "ribbon-queries",
                &[
                    (Icon::QueryWizard, "ribbon-query-wizard"),
                    (Icon::QueryDesign, "ribbon-query-design"),
                ],
            ),
            (
                "ribbon-forms",
                &[
                    (Icon::Form, "ribbon-form"),
                    (Icon::FormDesign, "ribbon-form-design"),
                    (Icon::BlankForm, "ribbon-blank-form"),
                    (Icon::FormWizard, "ribbon-form-wizard"),
                    (Icon::NavigationForm, "ribbon-navigation-form"),
                    (Icon::MoreForms, "ribbon-more-forms"),
                ],
            ),
            (
                "ribbon-reports",
                &[
                    (Icon::Report, "ribbon-report"),
                    (Icon::ReportDesign, "ribbon-report-design"),
                    (Icon::BlankReport, "ribbon-blank-report"),
                    (Icon::ReportWizard, "ribbon-report-wizard"),
                    (Icon::Labels, "ribbon-labels"),
                ],
            ),
            (
                "ribbon-macros-code",
                &[
                    (Icon::MacroCreate, "ribbon-macro"),
                    (Icon::Code, "ribbon-module"),
                    (Icon::ClassModule, "ribbon-class-module"),
                ],
            ),
        ],
        Tab::ExternalData => &[
            (
                "ribbon-import-link",
                &[
                    (Icon::Import, "ribbon-new-data-source"),
                    (Icon::SavedImports, "ribbon-saved-imports"),
                    (Icon::LinkedTables, "ribbon-linked-table-manager"),
                ],
            ),
            (
                "ribbon-export",
                &[
                    (Icon::Export, "ribbon-export"),
                    (Icon::SavedExports, "ribbon-saved-exports"),
                    (Icon::Spreadsheet, "ribbon-excel"),
                    (Icon::TextFile, "ribbon-text-file"),
                    (Icon::Xml, "ribbon-xml-file"),
                    (Icon::DatabaseFile, "ribbon-access-database"),
                    (Icon::PdfXps, "ribbon-pdf-xps"),
                    (Icon::Email, "ribbon-email"),
                    (Icon::WordMerge, "ribbon-word-merge"),
                    (Icon::OdbcDatabase, "ribbon-odbc-database"),
                    (Icon::SharePointList, "ribbon-sharepoint-list"),
                    (Icon::Export, "ribbon-export-more"),
                ],
            ),
        ],
        Tab::DatabaseTools => &[
            (
                "ribbon-tools",
                &[
                    (Icon::CompactRepair, "ribbon-compact-repair"),
                    (Icon::Code, "ribbon-visual-basic"),
                    (Icon::Run, "ribbon-run-macro"),
                ],
            ),
            (
                "ribbon-relationships",
                &[
                    (Icon::Relationships, "ribbon-relationships"),
                    (Icon::Dependencies, "ribbon-object-dependencies"),
                ],
            ),
            (
                "ribbon-analyze",
                &[
                    (Icon::Documenter, "ribbon-database-documenter"),
                    (Icon::Performance, "ribbon-analyze-performance"),
                    (Icon::AnalyzeTable, "ribbon-analyze-table"),
                ],
            ),
            (
                "ribbon-move-data",
                &[(Icon::DatabaseFile, "ribbon-access-database")],
            ),
            (
                "ribbon-add-ins",
                &[(Icon::ApplicationParts, "ribbon-add-ins")],
            ),
        ],
        Tab::Help => &[(
            "ribbon-help",
            &[
                (Icon::Help, "ribbon-help"),
                (Icon::Feedback, "ribbon-feedback"),
            ],
        )],
        Tab::TableFields => &[
            (
                "ribbon-views",
                &[
                    (Icon::ViewDatasheet, "ribbon-datasheet"),
                    (Icon::ViewDesign, "ribbon-table-design"),
                ],
            ),
            (
                "ribbon-add-delete",
                &[
                    (Icon::Add, "ribbon-new-field"),
                    (Icon::Delete, "ribbon-delete"),
                    (Icon::Properties, "ribbon-name-caption"),
                ],
            ),
            (
                "ribbon-properties",
                &[
                    (Icon::TableEdit, "ribbon-data-type"),
                    (Icon::TextColor, "ribbon-format"),
                    (Icon::Builder, "ribbon-default-value"),
                ],
            ),
            (
                "ribbon-validation",
                &[
                    (Icon::Validation, "ribbon-validation"),
                    (Icon::Key, "ribbon-required"),
                    (Icon::Indexes, "ribbon-unique"),
                    (Icon::Lookups, "ribbon-indexed"),
                ],
            ),
        ],
        Tab::Table => &[
            (
                "ribbon-properties",
                &[(Icon::Properties, "ribbon-table-properties")],
            ),
            (
                "ribbon-before-events",
                &[
                    (Icon::BeforeChange, "ribbon-before-change"),
                    (Icon::BeforeDelete, "ribbon-before-delete"),
                ],
            ),
            (
                "ribbon-after-events",
                &[
                    (Icon::AfterInsert, "ribbon-after-insert"),
                    (Icon::AfterUpdate, "ribbon-after-update"),
                    (Icon::AfterDelete, "ribbon-after-delete"),
                ],
            ),
            (
                "ribbon-macros-code",
                &[
                    (Icon::MacroCreate, "ribbon-create-data-macros"),
                    (Icon::MacroRename, "ribbon-rename-delete-macro"),
                ],
            ),
        ],
        Tab::Home | Tab::TableDesign | Tab::QueryDesign => return false,
    };

    super::strip(ui, |ui| {
        for (label, commands) in groups {
            group(ui, &t.text(label), |ui| {
                ui.horizontal(|ui| {
                    let large_count = if commands.len() <= 2 {
                        commands.len()
                    } else {
                        1
                    };
                    for (icon, key) in &commands[..large_count] {
                        unavailable(ui, t, *icon, key, CommandSize::Large);
                    }
                    for column in commands[large_count..].chunks(3) {
                        ui.vertical(|ui| {
                            for (icon, key) in column {
                                unavailable(ui, t, *icon, key, CommandSize::Small);
                            }
                        });
                    }
                });
            });
        }
    });
    true
}

fn describe_combo(ui: &Ui, response: &egui::Response, label: &str, value: &str, t: &Localizer) {
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(label);
        node.set_value(value);
        node.set_description(t.text("ribbon-unavailable"));
        node.remove_action(egui::accesskit::Action::Click);
        node.remove_action(egui::accesskit::Action::Focus);
    });
}

#[cfg(test)]
mod tests;
