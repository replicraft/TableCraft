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
    group(ui, &t.text("ribbon-sort-filter"), |ui| {
        ui.horizontal(|ui| {
            mock(ui, t, Icon::Filter, "ribbon-filter", CommandSize::Large);
            ui.vertical(|ui| {
                mock(
                    ui,
                    t,
                    Icon::Ascending,
                    "ribbon-ascending",
                    CommandSize::Small,
                );
                mock(
                    ui,
                    t,
                    Icon::Descending,
                    "ribbon-descending",
                    CommandSize::Small,
                );
                mock(
                    ui,
                    t,
                    Icon::RemoveSort,
                    "ribbon-remove-sort",
                    CommandSize::Small,
                );
            });
            ui.vertical(|ui| {
                mock(
                    ui,
                    t,
                    Icon::Selection,
                    "ribbon-selection",
                    CommandSize::Small,
                );
                mock(ui, t, Icon::Advanced, "ribbon-advanced", CommandSize::Small);
                mock(
                    ui,
                    t,
                    Icon::ToggleFilter,
                    "ribbon-toggle-filter",
                    CommandSize::Small,
                );
            });
        });
    });
    group(ui, &t.text("ribbon-records"), |ui| {
        ui.horizontal(|ui| {
            mock(
                ui,
                t,
                Icon::Refresh,
                "ribbon-refresh-all",
                CommandSize::Large,
            );
            ui.vertical(|ui| {
                mock(ui, t, Icon::Add, "ribbon-new", CommandSize::Small);
                mock(ui, t, Icon::Save, "ribbon-save", CommandSize::Small);
                mock(ui, t, Icon::Delete, "ribbon-delete", CommandSize::Small);
            });
            ui.vertical(|ui| {
                mock(ui, t, Icon::Sum, "ribbon-totals", CommandSize::Small);
                mock(ui, t, Icon::Spelling, "ribbon-spelling", CommandSize::Small);
                mock(ui, t, Icon::MoreRecords, "ribbon-more", CommandSize::Small);
            });
        });
    });
    group(ui, &t.text("ribbon-find-group"), |ui| {
        ui.horizontal(|ui| {
            mock(ui, t, Icon::Search, "ribbon-find", CommandSize::Large);
            ui.vertical(|ui| {
                mock(ui, t, Icon::Replace, "ribbon-replace", CommandSize::Small);
                mock(ui, t, Icon::GoTo, "ribbon-go-to", CommandSize::Small);
                mock(ui, t, Icon::Cursor, "ribbon-select", CommandSize::Small);
            });
        });
    });
    group(ui, &t.text("ribbon-text-formatting"), |ui| {
        ui.add_enabled(
            false,
            egui::Button::new(t.text("ribbon-font")).wrap_mode(egui::TextWrapMode::Extend),
        )
        .on_disabled_hover_text(t.text("ribbon-preview-only"));
        ui.horizontal(|ui| {
            mock(ui, t, Icon::Bold, "ribbon-bold", CommandSize::IconOnly);
            mock(ui, t, Icon::Italic, "ribbon-italic", CommandSize::IconOnly);
            mock(
                ui,
                t,
                Icon::Underline,
                "ribbon-underline",
                CommandSize::IconOnly,
            );
        });
        ui.horizontal(|ui| {
            mock(
                ui,
                t,
                Icon::TextColor,
                "ribbon-font-color",
                CommandSize::IconOnly,
            );
            mock(
                ui,
                t,
                Icon::Fill,
                "ribbon-fill-color",
                CommandSize::IconOnly,
            );
        });
    });
    group(ui, &t.text("ribbon-zoom"), |ui| {
        mock(ui, t, Icon::Zoom, "ribbon-zoom", CommandSize::Large);
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
