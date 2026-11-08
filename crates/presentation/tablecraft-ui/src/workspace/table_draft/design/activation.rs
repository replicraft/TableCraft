//! Resolve pending declarations before handing input to an existing field.
use super::{Editor, rename};
use egui::{Response, Ui};
use tablecraft_builders::table::DraftFieldId;

const CHEVRON_HALF_WIDTH: f32 = 3.0;
const CHEVRON_HALF_HEIGHT: f32 = 2.0;
const CHEVRON_GAP: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Selector,
    Name,
    Type,
    Description,
}

pub(super) struct Request {
    pub(super) field: DraftFieldId,
    pub(super) target: Target,
    pub(super) response: Response,
}

pub(super) fn apply(editor: &mut Editor, request: Request) -> bool {
    if !editor.commit_pending() {
        return false;
    }
    let Some(field) = editor
        .draft
        .fields()
        .iter()
        .find(|field| field.id() == request.field)
    else {
        return false;
    };
    editor.design.selected = Some(field.id());
    editor.new_field.deactivate();
    request.response.ctx.request_repaint();
    match request.target {
        Target::Selector => request.response.request_focus(),
        Target::Name => {
            if field.is_protected() {
                return false;
            }
            editor.rename = Some(rename::Rename::new(field.id(), field.name().into()));
        }
        Target::Type | Target::Description => {
            editor.design.pending_focus = Some((field.id(), request.target));
        }
    }
    true
}

// The proxy uses a non-focusable click sense so pointer activation cannot steal
// text intended for the new-name editor earlier in the same input batch.
pub(super) fn type_proxy(ui: &mut Ui, text: &str, label: &str) -> Response {
    let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::CLICK);
    let visuals = ui.style().interact(&response);
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let padding = ui.spacing().button_padding.x;
    let chevron = egui::pos2(rect.right() - padding - CHEVRON_HALF_WIDTH, rect.center().y);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            chevron + egui::vec2(-CHEVRON_HALF_WIDTH, -CHEVRON_HALF_HEIGHT),
            chevron + egui::vec2(CHEVRON_HALF_WIDTH, -CHEVRON_HALF_HEIGHT),
            chevron + egui::vec2(0.0, CHEVRON_HALF_HEIGHT),
        ],
        visuals.fg_stroke.color,
        egui::Stroke::NONE,
    ));
    let text_clip = egui::Rect::from_min_max(
        rect.min + egui::vec2(padding, 0.0),
        egui::pos2(chevron.x - CHEVRON_HALF_WIDTH - CHEVRON_GAP, rect.bottom()),
    )
    .intersect(ui.clip_rect());
    let galley = ui.painter().layout_no_wrap(
        text.into(),
        egui::TextStyle::Button.resolve(ui.style()),
        visuals.fg_stroke.color,
    );
    ui.painter().with_clip_rect(text_clip).galley(
        egui::pos2(text_clip.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        visuals.fg_stroke.color,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, response.enabled(), label)
    });
    response
}
