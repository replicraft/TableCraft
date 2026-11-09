//! Semantic containers for the visible portion of a virtualized datasheet.

use crate::Data;
use egui::{
    Ui, UiBuilder,
    accesskit::{Rect, Role},
};

pub(super) fn grid(ui: &mut Ui, data: &Data) -> Ui {
    let grid = ui.new_child(UiBuilder::new().id_salt("accessible_datasheet"));
    // An empty request beyond the end does not establish the relation's length.
    let known_end = !data.has_more && (data.offset == 0 || !data.rows.is_empty());
    let row_count = known_end
        .then(|| {
            usize::try_from(data.offset)
                .ok()?
                .checked_add(data.rows.len())?
                .checked_add(1)
        })
        .flatten();
    grid.ctx().accesskit_node_builder(grid.unique_id(), |node| {
        node.set_role(Role::Grid);
        // NO_CLONE: Two real toolkit outputs retain the text independently (paint/layout,
        // accessibility or hover/clipboard); the owning API consumes the String while
        // the other output remains needed.
        node.set_label(data.table.clone());
        node.set_column_count(data.columns.len() + 1);
        if let Some(count) = row_count {
            node.set_row_count(count);
        }
    });
    grid
}

pub(super) fn row(ui: &mut Ui, grid: &Ui, index: usize, offset: Option<u32>) -> Ui {
    let coordinate = match offset {
        Some(offset) => usize::try_from(offset)
            .ok()
            .and_then(|offset| offset.checked_add(index))
            .and_then(|index| index.checked_add(1)),
        None => Some(index),
    };
    let row = ui.new_child(
        UiBuilder::new()
            .id_salt(("accessible_row", offset, index))
            .accessibility_parent(grid.unique_id()),
    );
    row.ctx().accesskit_node_builder(row.unique_id(), |node| {
        node.set_role(Role::Row);
        if let Some(index) = coordinate {
            node.set_row_index(index);
        }
    });
    row
}

pub(super) fn cell(
    ui: &mut Ui,
    row: &mut Ui,
    column: usize,
    role: Role,
    contents: impl FnOnce(&mut Ui),
) {
    row.expand_to_include_rect(ui.max_rect());
    bounds(row);
    ui.scope_builder(
        UiBuilder::new()
            .id_salt("accessible_cell")
            .accessibility_parent(row.unique_id()),
        |ui| {
            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(role);
                node.set_column_index(column);
            });
            contents(ui);
            bounds(ui);
        },
    );
}

pub(super) fn bounds(ui: &Ui) {
    let rect = ui.min_rect();
    ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
        node.set_bounds(Rect {
            x0: f64::from(rect.left()),
            y0: f64::from(rect.top()),
            x1: f64::from(rect.right()),
            y1: f64::from(rect.bottom()),
        });
    });
}

#[cfg(test)]
mod tests;
