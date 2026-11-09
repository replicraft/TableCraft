//! Virtualized read-only text with real Copy, size and reviewed-edit controls.
use super::{Session, index};
use crate::localization::tr;
use crate::theme::palette;
use crate::workspace::{Action, DocumentView, State};
use egui::{FontId, Key, Label, Modifiers, Popup, Sense, Stroke, StrokeKind, Ui};

const MIN_WINDOW_SIZE: egui::Vec2 = egui::vec2(360.0, 260.0);
const DEFAULT_WINDOW_SIZE: egui::Vec2 = egui::vec2(680.0, 460.0);
const WINDOW_MARGIN: f32 = 48.0;
const FOOTER_RESERVE: f32 = 72.0;
const MIN_BODY_HEIGHT: f32 = 96.0;
const BOTTOM_PADDING_ROWS: usize = 1;
const HORIZONTAL_STEP_EMS: f32 = 4.0;
const TEXT_SIZES: [u8; 5] = [12, 14, 16, 18, 24];

impl State {
    pub(in crate::workspace) fn zoom_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(mut session) = self.zoom.session.take() else {
            return;
        };
        if self.busy || session.binding.cell(self).is_none() {
            return;
        }
        let mut text_size = self.zoom.text_size;
        let cell = session
            .binding
            .cell(self)
            .expect("current source-bound cell");
        let data = self.visible_data().expect("current source-bound data");
        let title = data.columns[session.binding.column].label();
        let edit = self.zoom_edit_target(&session.binding);
        let provenance = cell.context(&self.localizer);
        let query = self.view == DocumentView::Query;
        let mut open = true;
        let mut close = !Popup::is_any_open(context)
            && context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        let mut begin_edit = false;
        let mut failed = false;
        let maximum =
            (context.content_rect().size() - egui::Vec2::splat(WINDOW_MARGIN)).max(MIN_WINDOW_SIZE);
        let colors = palette(context);
        egui::Window::new(self.localizer.text("zoom-title"))
            .id(egui::Id::new("cell_zoom_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .min_size(MIN_WINDOW_SIZE)
            .max_size(maximum)
            .default_size(DEFAULT_WINDOW_SIZE.min(maximum))
            .default_pos(context.content_rect().center() - DEFAULT_WINDOW_SIZE.min(maximum) * 0.5)
            .frame(
                egui::Frame::window(&context.style_of(context.theme()))
                    .corner_radius(0)
                    .fill(colors.surface),
            )
            .order(egui::Order::Foreground)
            .show(context, |ui| {
                let body_height = (ui.available_height() - FOOTER_RESERVE).max(MIN_BODY_HEIGHT);
                egui::Frame::new()
                    .fill(colors.surface)
                    .stroke(Stroke::new(1.0, colors.grid_line))
                    .inner_margin(4.0)
                    .show(ui, |ui| {
                        failed = show_text(ui, &mut session, &cell.text, text_size, body_height)
                            .is_err();
                    })
                    .response
                    .on_hover_text(title);
                ui.small(tr(
                    ui,
                    if query {
                        "zoom-query-read-only"
                    } else {
                        "zoom-read-only"
                    },
                ))
                .on_hover_text(provenance.as_deref().unwrap_or(title));
                if session.copy_error {
                    ui.label(tr(ui, "zoom-copy-unavailable"));
                }
                if session.index.continued_lines {
                    ui.small(tr(ui, "zoom-continued"));
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.menu_button(tr(ui, "zoom-font"), |ui| {
                        ui.label("Selawik");
                        ui.label(tr(ui, "zoom-text-size"));
                        egui::ComboBox::from_id_salt("zoom_text_size")
                            .width(56.0)
                            .selected_text(text_size.to_string())
                            .show_ui(ui, |ui| {
                                for size in TEXT_SIZES {
                                    ui.selectable_value(&mut text_size, size, size.to_string());
                                }
                            });
                    });
                    if ui.button(tr(ui, "zoom-copy")).clicked() {
                        let mut copied = String::new();
                        session.copy_error = copied.try_reserve_exact(cell.text.len()).is_err();
                        if !session.copy_error {
                            copied.push_str(&cell.text);
                            context.copy_text(copied);
                        }
                    }
                    if edit.is_some() && ui.button(tr(ui, "zoom-edit")).clicked() {
                        begin_edit = true;
                    }
                    let ok = tr(ui, "zoom-ok");
                    let cancel = tr(ui, "zoom-cancel");
                    let padding = ui.spacing().button_padding.x * 2.0;
                    let button = |label: &str| {
                        ui.painter()
                            .layout_no_wrap(
                                label.to_owned(),
                                FontId::proportional(14.0),
                                colors.ink,
                            )
                            .size()
                            .x
                            + padding
                    };
                    let needed = button(&ok) + button(&cancel) + ui.spacing().item_spacing.x;
                    ui.add_space((ui.available_width() - needed).max(0.0));
                    close |= ui.button(ok).clicked();
                    close |= ui.button(cancel).clicked();
                });
            });
        self.zoom.text_size = text_size;
        if failed {
            self.error = Some(self.localizer.text("zoom-unavailable"));
        } else if begin_edit {
            if let Some(target) = edit
                && self.can_edit_cell(&target)
            {
                actions.push(Action::BeginEdit { target });
            }
        } else if close || !open {
            self.restore_zoom_focus(context, &session.binding);
        } else {
            self.zoom.session = Some(session);
        }
    }
}

fn show_text(
    ui: &mut Ui,
    session: &mut Session,
    value: &str,
    size: u8,
    height: f32,
) -> Result<(), index::IndexError> {
    let font = FontId::proportional(f32::from(size));
    let row_height = ui.fonts_mut(|fonts| fonts.row_height(&font)).ceil();
    let body_id = ui.id().with("zoom_text_body");
    let viewport_width = ui.available_width();
    let mut text_clicked = false;
    let mut failure = None;
    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut scroll = egui::ScrollArea::both()
        .id_salt("zoom_text_scroll")
        .auto_shrink([false, false])
        .min_scrolled_height(height)
        .max_height(height);
    if let Some(offset) = session.pending_scroll.take() {
        scroll = scroll.vertical_scroll_offset(offset);
    }
    if let Some(offset) = session.pending_horizontal.take() {
        scroll = scroll.horizontal_scroll_offset(offset);
    }
    // Leave one real visual row below the text. Large f32 scroll offsets can
    // otherwise round the final glyph a pixel beyond the viewport's bottom.
    let scroll_rows = session.index.row_count() + BOTTOM_PADDING_ROWS;
    let body = scroll.show_rows(ui, row_height, scroll_rows, |ui, requested| {
        let actual = requested.start.min(session.index.row_count())
            ..requested.end.min(session.index.row_count());
        if let Err(error) = session.visible.update(&session.index, value, actual) {
            failure = Some(error);
            return;
        }
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ui.set_min_width(session.max_width);
        for range in &session.visible.rows {
            // Range is a small non-Copy descriptor consumed by indexing/visible-row lookup
            // while the same original bounds remain cached or iterated; no underlying text is
            // copied.
            let Some(text) = value.get(range.clone()) else {
                failure = Some(index::IndexError::Changed);
                return;
            };
            let response = ui
                .allocate_ui_with_layout(
                    egui::vec2(viewport_width.max(session.max_width), row_height),
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        ui.add(
                            // The layout API consumes this FontId; keep the original style or
                            // loop-local descriptor available for the next measurement.
                            Label::new(egui::RichText::new(text).font(font.clone()))
                                .wrap_mode(egui::TextWrapMode::Extend)
                                .selectable(true),
                        )
                    },
                )
                .inner;
            text_clicked |= response.clicked() || response.drag_started();
            session.max_width = session.max_width.max(response.rect.width());
        }
        ui.set_min_width(session.max_width);
    });
    if let Some(error) = failure {
        return Err(error);
    }
    session.scroll_offset = body.state.offset.y;
    session.horizontal_offset = body.state.offset.x;
    let focus = ui.interact(body.inner_rect, body_id, Sense::focusable_noninteractive());
    let body_name = tr(ui, "zoom-read-only");
    focus.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), &body_name)
    });
    if session.focus_body || text_clicked {
        focus.request_focus();
        session.focus_body = false;
    }
    if focus.has_focus() {
        ui.painter().rect_stroke(
            body.inner_rect,
            0.0,
            Stroke::new(1.0, palette(ui.ctx()).accent),
            StrokeKind::Inside,
        );
        if !Popup::is_any_open(ui.ctx()) {
            let total = session.index.row_count() as f32 * row_height;
            let mut next = session.scroll_offset;
            let mut horizontal = session.horizontal_offset;
            ui.input_mut(|input| {
                for (key, delta) in [
                    (Key::ArrowDown, row_height),
                    (Key::ArrowUp, -row_height),
                    (Key::PageDown, body.inner_rect.height()),
                    (Key::PageUp, -body.inner_rect.height()),
                ] {
                    if input.consume_key(Modifiers::NONE, key) {
                        next += delta;
                    }
                }
                if input.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                    horizontal -= HORIZONTAL_STEP_EMS * f32::from(size);
                }
                if input.consume_key(Modifiers::NONE, Key::ArrowRight) {
                    horizontal += HORIZONTAL_STEP_EMS * f32::from(size);
                }
                if input.consume_key(Modifiers::NONE, Key::Home) {
                    next = 0.0;
                }
                if input.consume_key(Modifiers::NONE, Key::End) {
                    next = total;
                }
            });
            if next != session.scroll_offset {
                session.pending_scroll = Some(next.clamp(0.0, total));
                ui.ctx().request_repaint();
            }
            if horizontal != session.horizontal_offset {
                let maximum = (body.content_size.x - body.inner_rect.width()).max(0.0);
                session.pending_horizontal = Some(horizontal.clamp(0.0, maximum));
                ui.ctx().request_repaint();
            }
        }
    }
    Ok(())
}
