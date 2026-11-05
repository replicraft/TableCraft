//! Visual command previews. Every control is disabled and produces no action.
use super::{CommandSize, Icon, command, group};
use crate::Localizer;
use egui::Ui;

pub(in crate::workspace) fn mock(
    ui: &mut Ui,
    t: &Localizer,
    icon: Icon,
    key: &str,
    size: CommandSize,
) {
    command(ui, icon, &t.text(key), false, size)
        .on_disabled_hover_text(t.text("ribbon-preview-only"));
}

pub(in crate::workspace) fn home(ui: &mut Ui, t: &Localizer) {
    group(ui, &t.text("ribbon-text-formatting"), |ui| {
        ui.horizontal(|ui| {
            ui.add_enabled_ui(false, |ui| {
                egui::ComboBox::from_id_salt("mock_font_family")
                    .width(130.0)
                    .selected_text("Selawik")
                    .show_ui(ui, |ui| {
                        let _ = ui.selectable_label(true, "Selawik");
                    })
                    .response
                    .on_disabled_hover_text(t.text("ribbon-preview-only"));
            });
            ui.add_enabled_ui(false, |ui| {
                egui::ComboBox::from_id_salt("mock_font_size")
                    .width(44.0)
                    .selected_text("11")
                    .show_ui(ui, |ui| {
                        let _ = ui.selectable_label(true, "11");
                    })
                    .response
                    .on_disabled_hover_text(t.text("ribbon-preview-only"));
            });
            mock(
                ui,
                t,
                Icon::Bullets,
                "ribbon-bullets",
                CommandSize::IconOnly,
            );
            mock(
                ui,
                t,
                Icon::Numbering,
                "ribbon-numbering",
                CommandSize::IconOnly,
            );
            mock(
                ui,
                t,
                Icon::DecreaseIndent,
                "ribbon-decrease-indent",
                CommandSize::IconOnly,
            );
            mock(
                ui,
                t,
                Icon::IncreaseIndent,
                "ribbon-increase-indent",
                CommandSize::IconOnly,
            );
        });
        ui.horizontal(|ui| {
            for (icon, key) in [
                (Icon::Bold, "ribbon-bold"),
                (Icon::Italic, "ribbon-italic"),
                (Icon::Underline, "ribbon-underline"),
                (Icon::TextColor, "ribbon-font-color"),
                (Icon::Fill, "ribbon-fill-color"),
                (Icon::AlignLeft, "ribbon-align-left"),
                (Icon::AlignCenter, "ribbon-align-center"),
                (Icon::AlignRight, "ribbon-align-right"),
                (Icon::Borders, "ribbon-borders"),
                (Icon::AlternateRows, "ribbon-alternate-rows"),
            ] {
                mock(ui, t, icon, key, CommandSize::IconOnly);
            }
        });
    });
}

pub(in crate::workspace) fn design(ui: &mut Ui, t: &Localizer) {
    group(ui, &t.text("ribbon-tools"), |ui| {
        ui.horizontal(|ui| {
            mock(ui, t, Icon::Key, "ribbon-primary-key", CommandSize::Large);
            mock(ui, t, Icon::Builder, "ribbon-builder", CommandSize::Large);
            mock(
                ui,
                t,
                Icon::Validation,
                "ribbon-test-validation-rules",
                CommandSize::Large,
            );
            ui.vertical(|ui| {
                mock(
                    ui,
                    t,
                    Icon::InsertRows,
                    "ribbon-insert-rows",
                    CommandSize::Small,
                );
                mock(
                    ui,
                    t,
                    Icon::DeleteRows,
                    "ribbon-delete-rows",
                    CommandSize::Small,
                );
                mock(
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
            mock(
                ui,
                t,
                Icon::Properties,
                "ribbon-property-sheet",
                CommandSize::Large,
            );
            mock(ui, t, Icon::Indexes, "ribbon-indexes", CommandSize::Large);
        });
    });
    group(ui, &t.text("ribbon-field-record-table-events"), |ui| {
        ui.horizontal(|ui| {
            mock(
                ui,
                t,
                Icon::MacroCreate,
                "ribbon-create-data-macros",
                CommandSize::Large,
            );
            mock(
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
            mock(
                ui,
                t,
                Icon::Relationships,
                "ribbon-relationships",
                CommandSize::Large,
            );
            mock(
                ui,
                t,
                Icon::Dependencies,
                "ribbon-object-dependencies",
                CommandSize::Large,
            );
        });
    });
}

/// The additional tabs are selectable presentation previews, never services.
/// No retained Access capture currently qualifies these tab compositions.
pub(in crate::workspace) fn tab(ui: &mut Ui, t: &Localizer, tab: super::Tab) -> bool {
    use super::Tab;

    let groups: &[(&str, &[(Icon, &str)])] = match tab {
        Tab::Create => &[
            (
                "ribbon-tables",
                &[
                    (Icon::Table, "ribbon-table"),
                    (Icon::TableEdit, "ribbon-table-design"),
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
                        mock(ui, t, *icon, key, CommandSize::Large);
                    }
                    for column in commands[large_count..].chunks(3) {
                        ui.vertical(|ui| {
                            for (icon, key) in column {
                                mock(ui, t, *icon, key, CommandSize::Small);
                            }
                        });
                    }
                });
            });
        }
    });
    true
}
