//! Source cards expose exact metadata fields and explicit, owned join edges.
use super::{Session, click};
use crate::localization::tr;
use egui::{Frame, Rect, ScrollArea, Stroke, Ui};
use tablecraft_builders::query::{
    DesignColumn, DesignExpression, DesignField, DesignJoin, DesignJoinKind, DesignSource,
    MAX_DESIGN_COLUMNS, MAX_DESIGN_JOINS, draft_text_bytes,
};

const SOURCE_HEIGHT: f32 = 165.0;
const SOURCE_WIDTH: f32 = 170.0;
const SOURCE_HEADING_HEIGHT: f32 = 40.0;
const SOURCE_VIEWPORT_SHARE: f32 = 0.4;

#[derive(Default)]
struct SourceEvents {
    column: Option<DesignColumn>,
    join: Option<DesignJoin>,
    anchors: Vec<(DesignField, Rect)>,
    clicked: bool,
    pair: Option<click::Pair>,
}

pub(super) fn source_area(ui: &mut Ui, session: &mut Session) -> bool {
    let mut events = SourceEvents {
        pair: session.click_pair.take(),
        ..SourceEvents::default()
    };
    let height = (ui.available_height() * SOURCE_VIEWPORT_SHARE)
        .min(SOURCE_HEIGHT)
        .max(SOURCE_HEADING_HEIGHT + ui.spacing().interact_size.y);
    let mut joins_changed = false;
    ScrollArea::both()
        .id_salt("qbe_sources")
        .max_height(height)
        .min_scrolled_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for source in &session.draft.sources {
                    ui.push_id(("qbe_source", source.id), |ui| {
                        source_card(ui, session, source, &mut events, height);
                    });
                }
            });
            draw_joins(ui, session, &events.anchors);
            joins_changed = join_controls(ui, session);
        });

    if ui.is_enabled() && !events.clicked && ui.input(|input| input.pointer.any_released()) {
        events.pair = None;
    }
    session.click_pair = events.pair;
    let mut changed = false;
    if let Some(column) = events.column {
        let mut candidate = session.draft.clone();
        candidate.columns.push(column);
        if candidate.columns.len() <= MAX_DESIGN_COLUMNS && draft_text_bytes(&candidate).is_ok() {
            session.draft = candidate;
            changed = true;
        } else {
            session.error = Some(tr(ui, "qbe-limit"));
        }
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

fn source_card(
    ui: &mut Ui,
    session: &Session,
    source: &DesignSource,
    events: &mut SourceEvents,
    height: f32,
) {
    Frame::group(ui.style())
        .fill(crate::theme::palette(ui.ctx()).surface)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(SOURCE_WIDTH);
                ui.strong(source.alias.as_deref().unwrap_or(&source.table));
                ui.separator();
                ScrollArea::vertical()
                    .max_height(height - SOURCE_HEADING_HEIGHT)
                    .auto_shrink([false, false])
                    .show(ui, |ui| source_fields(ui, session, source, events));
            });
        });
}

fn source_fields(ui: &mut Ui, session: &Session, source: &DesignSource, events: &mut SourceEvents) {
    let all_fields = ui
        .push_id("all_fields", |ui| ui.selectable_label(false, "*"))
        .inner;
    if all_fields.clicked() {
        events.clicked = true;
    }
    if all_fields.clicked() && click::activate(ui, &all_fields, &mut events.pair) {
        events.column = Some(DesignColumn {
            expression: DesignExpression::AllFields(Some(source.id)),
            alias: None,
            show: true,
            sort: None,
            criteria: Vec::new(),
        });
    }
    let Some(metadata) = session
        .metadata
        .iter()
        .find(|table| table.table.eq_ignore_ascii_case(&source.table))
    else {
        ui.weak(tr(ui, "qbe-metadata-needed"));
        return;
    };
    for field in &metadata.fields {
        let reference = DesignField {
            source_id: source.id,
            column: field.name.clone(),
        };
        let response = ui
            .push_id(("field", &field.name), |ui| {
                ui.selectable_label(false, &field.name)
            })
            .inner;
        if response.rect.intersects(ui.clip_rect()) {
            events
                .anchors
                .push((reference.clone(), response.rect.intersect(ui.clip_rect())));
        }
        if response.clicked() {
            events.clicked = true;
            if click::activate(ui, &response, &mut events.pair) {
                events.column = Some(DesignColumn::field(source.id, field.name.clone()));
            }
        }
        response.context_menu(|ui| {
            join_menu(ui, session, &reference, events);
        });
    }
}

fn join_menu(ui: &mut Ui, session: &Session, left: &DesignField, events: &mut SourceEvents) {
    ui.menu_button(tr(ui, "qbe-join"), |ui| {
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
            ui.menu_button(source.alias.as_deref().unwrap_or(&source.table), |ui| {
                for field in &metadata.fields {
                    if ui.button(&field.name).clicked() {
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
        }
    });
}

fn draw_joins(ui: &Ui, session: &Session, anchors: &[(DesignField, Rect)]) {
    let stroke = Stroke::new(1.0, crate::theme::palette(ui.ctx()).muted);
    for join in &session.draft.joins {
        let left = anchors.iter().find(|(field, _)| *field == join.left);
        let right = anchors.iter().find(|(field, _)| *field == join.right);
        if let (Some((_, left)), Some((_, right))) = (left, right) {
            let (start, end) = if left.center().x < right.center().x {
                (left.right_center(), right.left_center())
            } else {
                (left.left_center(), right.right_center())
            };
            ui.painter().line_segment([start, end], stroke);
        }
    }
}

fn join_controls(ui: &mut Ui, session: &mut Session) -> bool {
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
