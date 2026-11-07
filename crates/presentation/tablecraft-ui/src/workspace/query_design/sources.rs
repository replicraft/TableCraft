//! Source cards expose exact metadata fields and explicit, owned join edges.
mod joins;
mod keyboard;

use super::{Session, click, context_menu};
use crate::localization::tr;
use egui::{Frame, Rect, Response, ScrollArea, Stroke, Ui};
use tablecraft_builders::query::{
    DesignColumn, DesignExpression, DesignField, DesignJoin, DesignJoinKind, DesignSource,
    MAX_DESIGN_COLUMNS, MAX_DESIGN_JOINS, draft_text_bytes,
};

const SOURCE_WIDTH: f32 = 170.0;
const SOURCE_HEADING_HEIGHT: f32 = 22.0;
const FIELD_ROW_HEIGHT: f32 = 18.0;
const CARD_GAP: f32 = 48.0;
const FIELD_TEXT_PADDING: f32 = 7.0;
const JOIN_LEAD: f32 = 12.0;
const CANVAS_MARGIN: i8 = 8;
const DEFAULT_CARD_HEIGHT: f32 = 160.0;

#[derive(Default)]
struct SourceEvents {
    column: Option<DesignColumn>,
    join: Option<DesignJoin>,
    anchors: Vec<(DesignField, Rect)>,
    clicked: bool,
    pair: Option<click::Pair>,
    outputs: Vec<(DesignExpression, keyboard::Output)>,
    selected_source: Option<u64>,
}

pub(super) fn source_area(ui: &mut Ui, session: &mut Session, height: f32) -> bool {
    let selection_id = ui.make_persistent_id("qbe_selected_source");
    let selected_source = ui
        .data(|data| data.get_temp::<u64>(selection_id))
        .filter(|selected| {
            session
                .draft
                .sources
                .iter()
                .any(|source| source.id == *selected)
        })
        .or_else(|| session.draft.sources.first().map(|source| source.id));
    let mut events = SourceEvents {
        pair: session.click_pair.take(),
        selected_source,
        ..SourceEvents::default()
    };
    let mut joins_changed = false;
    let canvas_height = (height - f32::from(CANVAS_MARGIN) * 2.0).max(FIELD_ROW_HEIGHT);
    let colors = crate::theme::palette(ui.ctx());
    Frame::new()
        .fill(colors.title_fill)
        .inner_margin(egui::Margin::same(CANVAS_MARGIN))
        .show(ui, |ui| {
            ScrollArea::both()
                .id_salt("qbe_sources")
                .max_height(canvas_height)
                .min_scrolled_height(canvas_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = CARD_GAP;
                        for source in &session.draft.sources {
                            ui.push_id(("qbe_source", source.id), |ui| {
                                source_card(
                                    ui,
                                    session,
                                    source,
                                    &mut events,
                                    canvas_height.min(DEFAULT_CARD_HEIGHT),
                                );
                            });
                        }
                    });
                    draw_joins(ui, session, &events.anchors);
                    joins_changed = joins::show(ui, session);
                });
        });

    if ui.is_enabled() && !events.clicked && ui.input(|input| input.pointer.any_released()) {
        events.pair = None;
    }
    session.click_pair = events.pair;
    if let Some(selected) = events.selected_source {
        ui.data_mut(|data| data.insert_temp(selection_id, selected));
    }
    let mut changed = false;
    for (expression, output) in events.outputs {
        let position = session.draft.columns.iter().position(|column| {
            keyboard::same_projection(&column.expression, &expression)
                && (output != keyboard::Output::Hide || column.show)
        });
        if let Some(position) = position {
            let column = &mut session.draft.columns[position];
            let show = match output {
                keyboard::Output::Show => true,
                keyboard::Output::Hide => false,
                keyboard::Output::Toggle => !column.show,
            };
            changed |= column.show != show;
            column.show = show;
        } else if output != keyboard::Output::Hide {
            changed |= append_column(
                ui,
                session,
                DesignColumn {
                    expression,
                    alias: None,
                    show: true,
                    sort: None,
                    criteria: Vec::new(),
                },
            );
        }
    }
    if let Some(column) = events.column {
        changed |= append_column(ui, session, column);
    }
    if let Some(join) = events.join {
        let duplicate = session.draft.joins.contains(&join);
        let mut candidate = session.draft.clone();
        candidate.joins.push(join);
        if !duplicate
            && candidate.joins.len() <= MAX_DESIGN_JOINS
            && draft_text_bytes(&candidate).is_ok()
        {
            session.draft = candidate;
            changed = true;
        } else {
            session.error = Some(tr(ui, "qbe-limit"));
        }
    }
    changed | joins_changed
}

fn append_column(ui: &Ui, session: &mut Session, column: DesignColumn) -> bool {
    let mut candidate = session.draft.clone();
    candidate.columns.push(column);
    if candidate.columns.len() <= MAX_DESIGN_COLUMNS && draft_text_bytes(&candidate).is_ok() {
        session.draft = candidate;
        true
    } else {
        session.error = Some(tr(ui, "qbe-limit"));
        false
    }
}

fn source_card(
    ui: &mut Ui,
    session: &Session,
    source: &DesignSource,
    events: &mut SourceEvents,
    height: f32,
) {
    let colors = crate::theme::palette(ui.ctx());
    Frame::new()
        .fill(colors.surface)
        .stroke(Stroke::new(1.0, colors.grid_line))
        .corner_radius(2.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(SOURCE_WIDTH);
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                Frame::new()
                    .fill(colors.grid_header)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        let title = source.alias.as_deref().unwrap_or(&source.table);
                        ui.add_sized(
                            [SOURCE_WIDTH - 12.0, SOURCE_HEADING_HEIGHT - 4.0],
                            egui::Label::new(egui::RichText::new(title).strong()).truncate(),
                        )
                        .on_hover_text(title);
                    });
                ScrollArea::vertical()
                    .id_salt("source_fields")
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .max_height((height - SOURCE_HEADING_HEIGHT - 2.0).max(FIELD_ROW_HEIGHT))
                    .auto_shrink([false, false])
                    .show(ui, |ui| source_fields(ui, session, source, events));
            });
        });
}

fn source_fields(ui: &mut Ui, session: &Session, source: &DesignSource, events: &mut SourceEvents) {
    let metadata = session
        .metadata
        .iter()
        .find(|table| table.table.eq_ignore_ascii_case(&source.table));
    let mut keyboard = keyboard::List::load(ui, source, metadata);
    let all_fields = ui
        .push_id("all_fields", |ui| field_row(ui, "*", keyboard.selected(0)))
        .inner;
    if all_fields.clicked_by(egui::PointerButton::Primary) {
        events.clicked = true;
    }
    if all_fields.clicked_by(egui::PointerButton::Primary)
        && click::activate(ui, &all_fields, &mut events.pair)
    {
        events.column = Some(DesignColumn {
            expression: DesignExpression::AllFields(Some(source.id)),
            alias: None,
            show: true,
            sort: None,
            criteria: Vec::new(),
        });
    }
    keyboard.row(all_fields, DesignExpression::AllFields(Some(source.id)));
    let Some(metadata) = metadata else {
        ui.weak(tr(ui, "qbe-metadata-needed"));
        keyboard.finish(ui, events);
        return;
    };
    for (index, field) in metadata.fields.iter().enumerate() {
        let reference = DesignField {
            source_id: source.id,
            column: field.name.clone(),
        };
        let response = ui
            .push_id(("field", &field.name), |ui| {
                field_row(ui, &field.name, keyboard.selected(index + 1))
            })
            .inner;
        if response.rect.intersects(ui.clip_rect()) {
            events
                .anchors
                .push((reference.clone(), response.rect.intersect(ui.clip_rect())));
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            events.clicked = true;
            if click::activate(ui, &response, &mut events.pair) {
                events.column = Some(DesignColumn::field(source.id, field.name.clone()));
            }
        }
        context_menu::show(ui, &response, |ui, keyboard_open| {
            join_menu(ui, session, &reference, events, keyboard_open);
        });
        keyboard.row(response, DesignExpression::Field(reference));
    }
    keyboard.finish(ui, events);
}

/// A compact full-width row keeps join anchors at card edges, never at the
/// end of a short field name. Full metadata names remain accessible and hoverable.
fn field_row(ui: &mut Ui, name: &str, selected: bool) -> Response {
    let sense = if selected {
        egui::Sense::click()
    } else {
        egui::Sense::CLICK
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), FIELD_ROW_HEIGHT), sense);
    let colors = crate::theme::palette(ui.ctx());
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 0.0, colors.active_row);
    }
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut layout = egui::text::LayoutJob::simple(
        name.to_owned(),
        font,
        colors.ink,
        (rect.width() - FIELD_TEXT_PADDING * 2.0).max(0.0),
    );
    layout.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(layout);
    ui.painter().with_clip_rect(rect).galley(
        egui::pos2(
            rect.left() + FIELD_TEXT_PADDING,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        colors.ink,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            name,
        )
    });
    if response.clicked() {
        response.request_focus();
    }
    response.on_hover_text(name)
}

fn join_menu(
    ui: &mut Ui,
    session: &Session,
    left: &DesignField,
    events: &mut SourceEvents,
    keyboard_open: bool,
) {
    let mut first_source = None;
    let join = ui.menu_button(tr(ui, "qbe-join"), |ui| {
        for source in &session.draft.sources {
            if source.id == left.source_id {
                continue;
            }
            let Some(metadata) = session
                .metadata
                .iter()
                .find(|table| table.table.eq_ignore_ascii_case(&source.table))
            else {
                continue;
            };
            let mut first_field = None;
            let source_menu =
                ui.menu_button(source.alias.as_deref().unwrap_or(&source.table), |ui| {
                    for field in &metadata.fields {
                        let target = ui.button(&field.name);
                        first_field.get_or_insert_with(|| target.clone());
                        if target.clicked() {
                            events.join = Some(DesignJoin {
                                left: left.clone(),
                                right: DesignField {
                                    source_id: source.id,
                                    column: field.name.clone(),
                                },
                                kind: DesignJoinKind::Inner,
                            });
                            ui.close();
                        }
                    }
                });
            if keyboard_activated(&source_menu.response)
                && let Some(target) = first_field
            {
                target.request_focus();
            }
            first_source.get_or_insert(source_menu.response);
        }
    });
    if keyboard_activated(&join.response)
        && let Some(target) = first_source
    {
        target.request_focus();
    }
    if keyboard_open {
        join.response.request_focus();
    }
}

fn keyboard_activated(response: &Response) -> bool {
    response.clicked() && !response.clicked_by(egui::PointerButton::Primary)
}

fn draw_joins(ui: &Ui, session: &Session, anchors: &[(DesignField, Rect)]) {
    let stroke = Stroke::new(1.0, crate::theme::palette(ui.ctx()).ink);
    for join in &session.draft.joins {
        let left = anchors.iter().find(|(field, _)| *field == join.left);
        let right = anchors.iter().find(|(field, _)| *field == join.right);
        if let (Some((_, left)), Some((_, right))) = (left, right) {
            let (start, end) = if left.center().x < right.center().x {
                (left.right_center(), right.left_center())
            } else {
                (left.left_center(), right.right_center())
            };
            let direction = (end.x - start.x).signum();
            let left_lead = start + egui::vec2(JOIN_LEAD * direction, 0.0);
            let right_lead = end - egui::vec2(JOIN_LEAD * direction, 0.0);
            ui.painter()
                .line(vec![start, left_lead, right_lead, end], stroke);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod menu_tests;
