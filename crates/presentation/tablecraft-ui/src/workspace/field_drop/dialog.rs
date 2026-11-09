//! Bounded deletion prompts adapted to existing application dialog composition.
//! No retained Access capture qualifies these menus/prompts/status appearances.
mod close;
mod confirmation;
mod panels;
mod status;

use egui::{Context, Ui};

const MAX_WIDTH: f32 = 480.0;
const MAX_REPORT_HEIGHT: f32 = 230.0;
const VIEWPORT_MARGIN: f32 = 32.0;
const FOOTER_ALLOWANCE: f32 = 110.0;

fn size(ui: &mut Ui, context: &Context) {
    ui.set_max_width(MAX_WIDTH.min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(100.0)));
}

fn body_height(context: &Context) -> f32 {
    MAX_REPORT_HEIGHT
        .min((context.viewport_rect().height() - VIEWPORT_MARGIN - FOOTER_ALLOWANCE).max(0.0))
}
