//! Existing explicit join-kind and removal controls.
use super::Session;
use crate::localization::tr;
use egui::Ui;
use tablecraft_builders::query::DesignJoinKind;

pub(super) fn show(ui: &mut Ui, session: &mut Session) -> bool {
    let mut changed = false;
    let mut remove = None;
    for (index, join) in session.draft.joins.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{} ↔ {}", join.left.column, join.right.column));
            egui::ComboBox::from_id_salt(("qbe_join_kind", index))
                .selected_text(tr(ui, join_key(join.kind)))
                .show_ui(ui, |ui| {
                    for kind in [
                        DesignJoinKind::Inner,
                        DesignJoinKind::Left,
                        DesignJoinKind::Right,
                    ] {
                        changed |= ui
                            .selectable_value(&mut join.kind, kind, tr(ui, join_key(kind)))
                            .changed();
                    }
                });
            if ui.button(tr(ui, "qbe-remove-join")).clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        session.draft.joins.remove(index);
        changed = true;
    }
    changed
}

fn join_key(kind: DesignJoinKind) -> &'static str {
    match kind {
        DesignJoinKind::Inner => "qbe-inner",
        DesignJoinKind::Left => "qbe-left",
        DesignJoinKind::Right => "qbe-right",
    }
}
