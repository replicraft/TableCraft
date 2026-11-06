//! Column geometry belongs to the object/source/schema, independently of page rows.
use crate::Data;
use egui::{Id, Ui};

#[derive(Clone, Default)]
struct Geometry {
    page: u32,
    widths: Vec<f32>,
    horizontal_offset: f32,
}

pub(super) struct PageLayout {
    id: Id,
    active_id: Id,
    geometry: Geometry,
    reset: bool,
}

impl PageLayout {
    pub(super) fn load(ui: &Ui, data: &Data) -> Self {
        let mut id =
            ui.make_persistent_id(("datasheet_geometry", data.source_revision, &data.table));
        id = id.with(data.columns.len());
        for column in &data.columns {
            id = id.with(&column.name);
        }
        let active_id = ui.make_persistent_id("datasheet_active_geometry");
        let previous = ui.data(|memory| memory.get_temp::<Id>(active_id));
        let existing = ui.data(|memory| memory.get_temp::<Geometry>(id));
        let reset = previous != Some(id)
            || existing
                .as_ref()
                .is_none_or(|geometry| geometry.page != data.offset);
        let mut geometry = existing.unwrap_or_default();
        geometry.page = data.offset;
        Self {
            id,
            active_id,
            geometry,
            reset,
        }
    }

    pub(super) fn reset_page(&self) -> bool {
        self.reset
    }

    pub(super) fn width(&self, index: usize) -> Option<f32> {
        self.geometry.widths.get(index).copied()
    }

    pub(super) fn horizontal_offset(&self) -> f32 {
        self.geometry.horizontal_offset
    }

    pub(super) fn observe_columns(&mut self, widths: &[f32], column_count: usize) {
        // The leading record selector and trailing add-field affordance are
        // fixed presentation controls, not user data-column widths.
        let Some(columns) = widths.get(1..=column_count) else {
            return;
        };
        if columns
            .iter()
            .any(|width| !width.is_finite() || *width <= 0.0)
        {
            return;
        }
        if self.geometry.widths != columns {
            self.geometry.widths.clear();
            self.geometry.widths.extend_from_slice(columns);
        }
    }

    pub(super) fn finish(mut self, ui: &Ui, horizontal_offset: f32) {
        if horizontal_offset.is_finite() {
            self.geometry.horizontal_offset = horizontal_offset.max(0.0);
        }
        ui.data_mut(|memory| {
            memory.insert_temp(self.id, self.geometry);
            memory.insert_temp(self.active_id, self.id);
        });
    }
}

#[cfg(test)]
mod tests;
