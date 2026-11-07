//! Existing explicit join-kind and removal controls.
use super::Session;
use crate::localization::{message, tr};
use egui::Ui;
use tablecraft_builders::query::{DesignField, DesignJoinKind, DesignSource};

pub(super) fn show(ui: &mut Ui, session: &mut Session) -> bool {
    let mut changed = false;
    let mut remove = None;
    for (index, join) in session.draft.joins.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let left = field_label(&session.draft.sources, &join.left);
            let right = field_label(&session.draft.sources, &join.right);
            let endpoints = [("left", left), ("right", right)];
            ui.label(message(ui, "qbe-join-endpoints", &endpoints));
            let kind_label = message(ui, "qbe-join-kind", &endpoints);
            let remove_label = message(ui, "qbe-remove-join-accessible", &endpoints);
            let kind = egui::ComboBox::from_id_salt(("qbe_join_kind", index))
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
            ui.ctx().accesskit_node_builder(kind.response.id, |node| {
                node.set_label(kind_label);
            });
            let removal = ui.button(tr(ui, "qbe-remove-join"));
            ui.ctx().accesskit_node_builder(removal.id, |node| {
                node.set_label(remove_label);
            });
            if removal.clicked() {
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

fn field_label(sources: &[DesignSource], field: &DesignField) -> String {
    let source = sources.iter().find(|source| source.id == field.source_id);
    let name = source
        .map(|source| source.alias.as_deref().unwrap_or(&source.table).to_owned())
        .unwrap_or_else(|| format!("#{}", field.source_id));
    // This is literal presentation text, never interpolated back into SQL.
    format!(
        "[{}].[{}]",
        name.replace(']', "]]"),
        field.column.replace(']', "]]")
    )
}

#[cfg(test)]
mod tests;
