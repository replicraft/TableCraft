//! UI-only expanded and tabs-only Ribbon presentation.
#[cfg(test)]
mod tests;

use super::{Action, State, focus, palette};
use crate::workspace::ribbon;
use egui::{Id, Key, Modifiers, Popup, Rect, Response, Ui, WidgetInfo, WidgetType};

const TOGGLE_SIZE: f32 = 18.0;
pub(super) const TOGGLE_GUTTER: i8 = 24;
const OVERLAY_MARKER: &str = "workspace_temporary_ribbon";

#[derive(Default)]
pub(in crate::workspace) struct RibbonDisplay {
    collapsed: bool,
    temporary: bool,
    scope: Option<Id>,
    focus_tab: bool,
    frame: Option<DisplayFrame>,
    tab_pair: Option<TabPair>,
    tab_clicked: bool,
}

struct TabPair {
    tab: ribbon::Tab,
    time: f64,
    position: egui::Pos2,
}

struct DisplayFrame {
    number: u64,
    tabs: Rect,
    layer: egui::LayerId,
    // Comparison only: a newer user focus choice may dismiss the temporary UI.
    focus_before: Option<Id>,
}

#[derive(Clone, Default)]
struct OverlayMarker {
    frame: u64,
}

impl State {
    /// Whether the session preference retains tabs without the command strip.
    pub fn ribbon_collapsed(&self) -> bool {
        self.ribbon_display.collapsed
    }

    /// Whether the existing commands are temporarily overlaid below the tabs.
    pub fn ribbon_temporarily_open(&self) -> bool {
        self.ribbon_display.temporary
    }

    /// Set only a presentation preference; no database operation is requested.
    pub fn set_ribbon_collapsed(&mut self, collapsed: bool) {
        self.ribbon_display.collapsed = collapsed;
        self.ribbon_display.temporary = false;
        self.ribbon_display.scope = None;
        self.ribbon_display.tab_pair = None;
    }

    /// Presentation fixtures may expose existing commands without requesting I/O.
    pub fn show_ribbon_temporarily(&mut self) {
        if self.ribbon_display.collapsed && !self.backstage_visible() && !self.edit_dialog_open() {
            self.ribbon_display.temporary = true;
        }
    }

    pub(in crate::workspace) fn prepare_ribbon_display(&mut self, ui: &Ui, modal: bool) {
        self.ribbon_display.frame = None;
        self.ribbon_display.tab_clicked = false;
        let available =
            ui.is_enabled() && !modal && !self.backstage_visible() && !self.find_window_open();
        let scope = Id::new((
            self.database
                .as_ref()
                .map(|database| database.document_revision),
            self.active_object().map(|object| object.id),
            match self.view {
                super::DocumentView::Table => 0,
                super::DocumentView::Design => 1,
                super::DocumentView::Query => 2,
            },
            self.query_design_context(),
        ));
        if !available
            || self
                .ribbon_display
                .scope
                .is_some_and(|previous| previous != scope)
        {
            self.ribbon_display.temporary = false;
            self.ribbon_display.tab_pair = None;
        }
        self.ribbon_display.scope = Some(scope);
        if available && ui.input_mut(|input| input.consume_key(Modifiers::CTRL, Key::F1)) {
            self.ribbon_display.collapsed = !self.ribbon_display.collapsed;
            self.ribbon_display.temporary = false;
            self.ribbon_display.focus_tab = true;
            self.ribbon_display.tab_pair = None;
        }
        // A child menu owns the first Escape. Consume the Ribbon's Escape before
        // the workspace can reinterpret it as cancellation of background work.
        if available
            && self.ribbon_display.temporary
            && !Popup::is_any_open(ui.ctx())
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.ribbon_display.temporary = false;
            self.ribbon_display.focus_tab = true;
        }
        self.clear_closed_ribbon_popups(ui);
    }

    fn clear_closed_ribbon_popups(&self, ui: &Ui) {
        if self.ribbon_display.temporary {
            return;
        }
        let marker = ui.data_mut(|data| data.remove_temp::<OverlayMarker>(Id::new(OVERLAY_MARKER)));
        if marker
            .is_some_and(|marker| marker.frame.saturating_add(1) >= ui.ctx().cumulative_frame_nr())
        {
            Popup::close_all(ui.ctx());
        }
    }

    pub(super) fn ribbon_tab_response(&mut self, response: &Response, candidate: ribbon::Tab) {
        if !response.enabled() {
            return;
        }
        let paired = response.clicked() && self.ribbon_tab_pair(response, candidate);
        if paired {
            self.ribbon_display.collapsed = !self.ribbon_display.collapsed;
            self.ribbon_display.temporary = false;
        } else if response.clicked() && self.ribbon_display.collapsed {
            self.ribbon_display.temporary =
                !self.ribbon_display.temporary || self.ribbon_tab != candidate;
        }
        if response.clicked() {
            self.ribbon_tab = candidate;
        }
        if self.ribbon_display.focus_tab && self.ribbon_tab == candidate {
            response.request_focus();
            self.ribbon_display.focus_tab = false;
        }
    }

    fn ribbon_tab_pair(&mut self, response: &Response, candidate: ribbon::Tab) -> bool {
        self.ribbon_display.tab_clicked = true;
        let Some(position) = response.interact_pointer_pos() else {
            self.ribbon_display.tab_pair = None;
            return false;
        };
        let time = response.ctx.input(|input| input.time);
        let (delay, distance) = response.ctx.options(|options| {
            (
                options.input_options.max_double_click_delay,
                options.input_options.max_click_dist,
            )
        });
        let paired = self.ribbon_display.tab_pair.take().is_some_and(|previous| {
            previous.tab == candidate
                && (0.0..delay).contains(&(time - previous.time))
                && previous.position.distance_sq(position) < distance * distance
        });
        if !paired {
            self.ribbon_display.tab_pair = Some(TabPair {
                tab: candidate,
                time,
                position,
            });
        }
        paired
    }

    pub(super) fn finish_ribbon_tab_clicks(&mut self, ui: &Ui) {
        if !self.ribbon_display.tab_clicked && ui.input(|input| input.pointer.any_released()) {
            self.ribbon_display.tab_pair = None;
        }
    }

    pub(super) fn show_ribbon_commands(
        &mut self,
        ui: &mut Ui,
        tabs: Rect,
        actions: &mut Vec<Action>,
    ) {
        if self.backstage_visible() {
            self.ribbon_display.temporary = false;
            return;
        }
        if !self.ribbon_display.collapsed {
            egui::Panel::top("ribbon")
                .frame(
                    egui::Frame::new()
                        .fill(palette(ui.ctx()).title_fill)
                        .inner_margin(egui::Margin {
                            left: 4,
                            right: 4,
                            top: 0,
                            bottom: 4,
                        }),
                )
                .show(ui, |ui| {
                    self.command_frame(ui, false, actions);
                });
        } else if self.ribbon_display.temporary && ui.is_enabled() {
            let before_actions = actions.len();
            let before_output = ui.ctx().output(|output| output.commands.len());
            let previous_menu = Popup::is_any_open(ui.ctx());
            let before_paste = self.paste_request.is_some();
            let focus_before = ui.ctx().memory(|memory| memory.focused());
            let response = egui::Area::new(Id::new(OVERLAY_MARKER))
                .order(egui::Order::Foreground)
                .fixed_pos(tabs.left_bottom())
                .movable(false)
                .show(ui.ctx(), |ui| {
                    ui.set_width(tabs.width());
                    self.command_frame(ui, true, actions);
                });
            self.ribbon_display.frame = Some(DisplayFrame {
                number: ui.ctx().cumulative_frame_nr(),
                tabs,
                layer: response.response.layer_id,
                focus_before,
            });
            let current_frame = ui.ctx().cumulative_frame_nr();
            ui.data_mut(|data| {
                data.insert_temp(
                    Id::new(OVERLAY_MARKER),
                    OverlayMarker {
                        frame: current_frame,
                    },
                )
            });
            let accepted = actions.len() != before_actions
                || ui.ctx().output(|output| output.commands.len()) != before_output
                || (!before_paste && self.paste_request.is_some())
                || self.edit_dialog_open()
                || self.find_window_open()
                || self.zoom_dialog_open()
                || self.backstage_visible()
                || previous_menu
                    && !Popup::is_any_open(ui.ctx())
                    && !ui.input(|input| input.key_pressed(Key::Escape));
            let outside = !Popup::is_any_open(ui.ctx())
                && ui.input(|input| {
                    input.pointer.any_pressed()
                        && input.pointer.interact_pos().is_some_and(|position| {
                            !tabs.contains(position) && !response.response.rect.contains(position)
                        })
                });
            if accepted || outside {
                self.ribbon_display.temporary = false;
            }
        }
        self.clear_closed_ribbon_popups(ui);
    }

    fn command_frame(&mut self, ui: &mut Ui, temporary: bool, actions: &mut Vec<Action>) {
        let colors = palette(ui.ctx());
        let frame = egui::Frame::new()
            .fill(colors.surface)
            .corner_radius(5)
            .stroke(egui::Stroke::new(1.0, colors.grid_line))
            .inner_margin(egui::Margin {
                left: 8,
                right: TOGGLE_GUTTER,
                top: 3,
                bottom: 3,
            })
            .show(ui, |ui| self.home(ui, actions));
        let mode = if temporary {
            ToggleMode::Pin
        } else {
            ToggleMode::Collapse
        };
        if toggle(ui, frame.response.rect, mode, &self.localizer).clicked() {
            self.ribbon_display.collapsed = !temporary;
            self.ribbon_display.temporary = false;
        }
    }

    pub(super) fn collapsed_ribbon_toggle(&mut self, ui: &mut Ui, tabs: Rect) {
        if self.ribbon_display.collapsed
            && !self.ribbon_display.temporary
            && toggle(ui, tabs, ToggleMode::Expand, &self.localizer).clicked()
        {
            self.set_ribbon_collapsed(false);
        }
    }

    pub(in crate::workspace) fn finish_ribbon_display(&mut self, ui: &Ui) {
        let Some(frame) = &self.ribbon_display.frame else {
            return;
        };
        if !self.ribbon_display.temporary || frame.number != ui.ctx().cumulative_frame_nr() {
            return;
        }
        let focused = ui.ctx().memory(|memory| memory.focused());
        if !Popup::is_any_open(ui.ctx())
            && focused != frame.focus_before
            && focused
                .and_then(|id| ui.ctx().read_response(id))
                .is_some_and(|response| {
                    !frame.tabs.intersects(response.rect) && response.layer_id != frame.layer
                })
        {
            self.ribbon_display.temporary = false;
        }
        self.clear_closed_ribbon_popups(ui);
    }
}

#[derive(Clone, Copy)]
enum ToggleMode {
    Collapse,
    Expand,
    Pin,
}

fn toggle(ui: &mut Ui, bounds: Rect, mode: ToggleMode, localizer: &crate::Localizer) -> Response {
    let rect = Rect::from_min_size(
        egui::pos2(
            bounds.right() - TOGGLE_SIZE - 3.0,
            bounds.bottom() - TOGGLE_SIZE - 1.0,
        ),
        egui::Vec2::splat(TOGGLE_SIZE),
    );
    let response = ui.interact(
        rect,
        ui.make_persistent_id("ribbon_display_toggle"),
        egui::Sense::click(),
    );
    let label = localizer.text(match mode {
        ToggleMode::Collapse => "ribbon-collapse",
        ToggleMode::Expand => "ribbon-expand",
        ToggleMode::Pin => "ribbon-pin",
    });
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    focus::register(ui, super::Pane::Ribbon, &response, false);
    let colors = palette(ui.ctx());
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 2.0, colors.active_header);
    }
    let center = rect.center();
    let points = if matches!(mode, ToggleMode::Pin) {
        vec![
            center + egui::vec2(-4.0, -4.0),
            center + egui::vec2(3.0, -4.0),
            center + egui::vec2(1.0, 0.0),
            center + egui::vec2(4.0, 2.0),
            center + egui::vec2(-4.0, 2.0),
            center + egui::vec2(-1.0, 0.0),
        ]
    } else {
        let direction = if matches!(mode, ToggleMode::Expand) {
            -1.0
        } else {
            1.0
        };
        vec![
            center + egui::vec2(-3.0, 1.5 * direction),
            center + egui::vec2(0.0, -1.5 * direction),
            center + egui::vec2(3.0, 1.5 * direction),
        ]
    };
    ui.painter()
        .line(points, egui::Stroke::new(1.0, colors.ink));
    if matches!(mode, ToggleMode::Pin) {
        ui.painter().line_segment(
            [center + egui::vec2(0.0, 2.0), center + egui::vec2(0.0, 6.0)],
            egui::Stroke::new(1.0, colors.ink),
        );
    }
    response.on_hover_text(label)
}
