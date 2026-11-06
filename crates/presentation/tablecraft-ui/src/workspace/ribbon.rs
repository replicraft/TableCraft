//! Dense Access-style command groups; mock commands remain disabled and inert.
mod icons;
mod mock_groups;

pub(super) use super::focus::{self, Pane};
pub(crate) use icons::Icon;
pub(super) use mock_groups::{design, home, mock as mock_command, tab as mock_tab};

use crate::theme::palette;
use egui::{FontId, Response, Sense, Ui, WidgetInfo, WidgetType};

// Two-line menu labels and their arrows share one baseline across groups.
const COMMAND_AREA_HEIGHT: f32 = 68.0;
const LARGE_COMMAND_WIDTH: f32 = 54.0;
const SMALL_COMMAND_HEIGHT: f32 = 20.0;
const LARGE_ICON_SIZE: f32 = 28.0;
const SMALL_ICON_SIZE: f32 = 16.0;
const LABEL_SIZE: f32 = 11.5;
const GROUP_LABEL_HEIGHT: f32 = 14.0;
const GROUP_LABEL_SIZE: f32 = 10.5;
const GROUP_LABEL_PADDING: f32 = 8.0;
const SCROLLBAR_THICKNESS: f32 = 6.0;
const GROUP_LAUNCHER_SIZE: f32 = 12.0;
const GROUP_LAUNCHER_GAP: f32 = 2.0;
const MENU_ARROW_SPACE: f32 = 12.0;
const MENU_ARROW_HALF_WIDTH: f32 = 2.5;
const LARGE_TEXT_TOP: f32 = 31.0;
const MENU_ARROW_VERTICAL_SPACE: f32 = 6.0;
const MENU_ARROW_BOTTOM_PADDING: f32 = 3.5;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Tab {
    #[default]
    Home,
    TableDesign,
    QueryDesign,
    Create,
    ExternalData,
    DatabaseTools,
    Help,
    TableFields,
    Table,
}

impl Tab {
    pub(super) fn mock(key: &str) -> Option<Self> {
        match key {
            "ribbon-create" => Some(Self::Create),
            "ribbon-external-data" => Some(Self::ExternalData),
            "ribbon-database-tools" => Some(Self::DatabaseTools),
            "ribbon-help" => Some(Self::Help),
            "ribbon-table-fields" => Some(Self::TableFields),
            "ribbon-table" => Some(Self::Table),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum CommandSize {
    Small,
    Large,
    IconOnly,
}

pub(super) fn strip(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().scroll.floating = false;
        ui.spacing_mut().scroll.bar_width = SCROLLBAR_THICKNESS;
        egui::ScrollArea::horizontal()
            .id_salt("ribbon_commands")
            .auto_shrink([false, true])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    contents(ui);
                });
            });
    });
}

pub(super) fn group(ui: &mut Ui, label: &str, contents: impl FnOnce(&mut Ui)) {
    group_with_launcher(ui, label, false, contents);
}

pub(super) fn group_with_launcher(
    ui: &mut Ui,
    label: &str,
    preview_launcher: bool,
    contents: impl FnOnce(&mut Ui),
) {
    let colors = palette(ui.ctx());
    let caption = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(GROUP_LABEL_SIZE),
        colors.muted,
    );
    ui.vertical(|ui| {
        // Reserve the translated caption before laying out narrow command groups.
        // Measuring it afterwards lets Extend paint into the adjacent group.
        let launcher_width = if preview_launcher {
            GROUP_LAUNCHER_SIZE + GROUP_LAUNCHER_GAP
        } else {
            0.0
        };
        ui.set_min_width(caption.size().x + GROUP_LABEL_PADDING + launcher_width);
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.scope(|ui| {
            ui.set_min_height(COMMAND_AREA_HEIGHT);
            contents(ui);
        });
        let width = ui.min_rect().width();
        let (caption_rect, caption_response) =
            ui.allocate_exact_size(egui::vec2(width, GROUP_LABEL_HEIGHT), Sense::hover());
        let text_width = width - launcher_width;
        let text_position = egui::pos2(
            caption_rect.left() + (text_width - caption.size().x) * 0.5,
            caption_rect.center().y - caption.size().y * 0.5,
        );
        ui.painter().galley(text_position, caption, colors.muted);
        caption_response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), label));
        if preview_launcher {
            let rect = egui::Rect::from_min_size(
                egui::pos2(
                    caption_rect.right() - GROUP_LAUNCHER_SIZE,
                    caption_rect.top(),
                ),
                egui::vec2(GROUP_LAUNCHER_SIZE, GROUP_LAUNCHER_SIZE),
            );
            let help = crate::localization::tr(ui, "ribbon-preview-only");
            let name = format!(
                "{label}: {}",
                crate::localization::tr(ui, "ribbon-group-options")
            );
            ui.add_enabled_ui(false, |ui| {
                ui.set_opacity(1.0);
                // A standard Button enforces its own padding/minimum size,
                // expanding this native-size launcher into the caption. Keep
                // the actual disabled hit region and artwork inside the gutter.
                let response = ui.interact(
                    rect,
                    ui.make_persistent_id(("group_launcher", label)),
                    Sense::hover(),
                );
                let image = egui::Rect::from_center_size(rect.center(), egui::vec2(10.0, 10.0));
                Icon::DialogLauncher.image(ui, 10.0).paint_at(ui, image);
                response
                    .widget_info(|| WidgetInfo::labeled(WidgetType::Button, false, name.clone()));
                ui.ctx().accesskit_node_builder(response.id, |node| {
                    node.remove_action(egui::accesskit::Action::Click);
                    node.remove_action(egui::accesskit::Action::Focus);
                });
                response.on_disabled_hover_text(help);
            });
        }
    });
    ui.separator();
}

pub(super) fn command(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    size: CommandSize,
) -> Response {
    command_inner(ui, icon, label, enabled, size, false)
}

/// Only real popup consumers use this affordance; it creates no menu or action.
pub(super) fn command_menu(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    size: CommandSize,
) -> Response {
    let response = command_inner(ui, icon, label, enabled, size, true);
    let popup = egui::Popup::default_response_id(&response);
    if !response.enabled() {
        egui::Popup::close_id(ui.ctx(), popup);
    }
    let expanded = egui::Popup::is_id_open(ui.ctx(), popup);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_has_popup(egui::accesskit::HasPopup::Menu);
        node.set_expanded(expanded);
    });
    response
}

fn command_inner(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    size: CommandSize,
    menu: bool,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        if !ui.is_enabled() {
            // Neutral artwork and muted text already mark unavailable commands.
            // Preserve their contrast instead of applying a second opacity fade.
            ui.set_opacity(1.0);
        }
        let colors = palette(ui.ctx());
        let color = if ui.is_enabled() {
            colors.ink
        } else {
            colors.muted
        };
        let font = FontId::proportional(LABEL_SIZE);
        let (mut desired, text_width) = match size {
            CommandSize::Large => {
                let width = large_command_width(ui, label, &font, color);
                (egui::vec2(width, COMMAND_AREA_HEIGHT), width - 6.0)
            }
            CommandSize::Small => {
                let text = ui
                    .painter()
                    .layout_no_wrap(label.to_owned(), font.clone(), color);
                (
                    egui::vec2(SMALL_ICON_SIZE + text.size().x + 10.0, SMALL_COMMAND_HEIGHT),
                    f32::INFINITY,
                )
            }
            CommandSize::IconOnly => (egui::vec2(24.0, SMALL_COMMAND_HEIGHT), 0.0),
        };
        let mut layout = egui::text::LayoutJob::simple(label.to_owned(), font, color, text_width);
        layout.wrap.max_rows = match size {
            CommandSize::Large => 2,
            CommandSize::Small | CommandSize::IconOnly => 1,
        };
        let text = ui.painter().layout_job(layout);
        if menu {
            match size {
                CommandSize::Large => {
                    desired.y = desired
                        .y
                        .max(LARGE_TEXT_TOP + text.size().y + MENU_ARROW_VERTICAL_SPACE);
                }
                CommandSize::Small | CommandSize::IconOnly => desired.x += MENU_ARROW_SPACE,
            }
        }
        let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
        focus::register(ui, Pane::Ribbon, &response, false);
        if response.enabled() && response.gained_focus() {
            response.scroll_to_me(None);
        }

        if ui.is_enabled() && (response.hovered() || response.has_focus()) {
            let visuals = ui.style().interact(&response);
            ui.painter().rect_filled(rect, 2.0, visuals.weak_bg_fill);
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    egui::Stroke::new(1.0, colors.accent),
                    egui::StrokeKind::Inside,
                );
            }
        }
        let (image_rect, text_position) = match size {
            CommandSize::Large => (
                egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + LARGE_ICON_SIZE * 0.5 + 3.0),
                    egui::vec2(LARGE_ICON_SIZE, LARGE_ICON_SIZE),
                ),
                egui::pos2(
                    rect.center().x - text.size().x * 0.5,
                    rect.top() + LARGE_TEXT_TOP,
                ),
            ),
            CommandSize::Small => (
                egui::Rect::from_center_size(
                    egui::pos2(rect.left() + SMALL_ICON_SIZE * 0.5 + 2.0, rect.center().y),
                    egui::vec2(SMALL_ICON_SIZE, SMALL_ICON_SIZE),
                ),
                egui::pos2(
                    rect.left() + SMALL_ICON_SIZE + 6.0,
                    rect.center().y - text.size().y * 0.5,
                ),
            ),
            CommandSize::IconOnly => (
                egui::Rect::from_center_size(
                    rect.center(),
                    egui::vec2(SMALL_ICON_SIZE, SMALL_ICON_SIZE),
                ),
                rect.center(),
            ),
        };
        icon.image(ui, image_rect.width()).paint_at(ui, image_rect);
        if !matches!(size, CommandSize::IconOnly) {
            ui.painter().galley(text_position, text, color);
        }
        if menu {
            let center = match size {
                CommandSize::Large => {
                    egui::pos2(rect.center().x, rect.bottom() - MENU_ARROW_BOTTOM_PADDING)
                }
                CommandSize::Small | CommandSize::IconOnly => {
                    egui::pos2(rect.right() - MENU_ARROW_SPACE * 0.5, rect.center().y)
                }
            };
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    center + egui::vec2(-MENU_ARROW_HALF_WIDTH, -1.5),
                    center + egui::vec2(MENU_ARROW_HALF_WIDTH, -1.5),
                    center + egui::vec2(0.0, 1.5),
                ],
                color,
                egui::Stroke::NONE,
            ));
        }
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
        if !response.enabled() {
            // Disabled previews have no callable or focusable command contract,
            // including when an assistive host sends an unsolicited request.
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.remove_action(egui::accesskit::Action::Click);
                node.remove_action(egui::accesskit::Action::Focus);
            });
        }
        response
    })
    .inner
}

fn large_command_width(ui: &Ui, label: &str, font: &FontId, color: egui::Color32) -> f32 {
    let measure = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), color)
            .size()
            .x
    };
    let mut text_width = measure(label);

    // Choose a real word boundary for two complete lines. A fixed English-sized
    // button can otherwise elide translated commands even when two lines fit.
    for (offset, character) in label.char_indices() {
        if character.is_whitespace() {
            let first = label[..offset].trim_end();
            let second = label[offset..].trim_start();
            if !first.is_empty() && !second.is_empty() {
                text_width = text_width.min(measure(first).max(measure(second)));
            }
        }
    }

    LARGE_COMMAND_WIDTH.max(text_width.ceil() + 8.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::emath::GuiRounding;

    fn frame(
        context: &egui::Context,
        events: Vec<egui::Event>,
    ) -> (egui::Id, egui::Rect, egui::Rect, Option<egui::Rect>) {
        let mut observation = None;
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 120.0),
                )),
                time: Some(context.cumulative_frame_nr() as f64 * 0.25),
                events,
                ..Default::default()
            },
            |ui| {
                strip(ui, |ui| {
                    ui.add_space(360.0);
                    let response = command(ui, Icon::Copy, "Copy", true, CommandSize::Small);
                    observation = Some((response.id, response.rect, ui.clip_rect()));
                });
            },
        );
        let text_rect = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "Copy" => {
                Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
            }
            _ => None,
        });
        output.textures_delta.clear();
        let (id, rect, clip) = observation.expect("actual overflow command geometry");
        (id, rect, clip, text_rect)
    }

    #[test]
    fn gaining_focus_reveals_an_overflow_command_without_overriding_later_manual_scroll() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let context = egui::Context::default();
            context.set_pixels_per_point(scale);
            context.enable_accesskit();
            crate::configure(&context);
            frame(&context, vec![]);
            assert_eq!(context.pixels_per_point(), scale);
            let (id, before, clip, _) = frame(&context, vec![]);
            assert!(before.left() > clip.right());

            frame(
                &context,
                vec![egui::Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action: egui::accesskit::Action::Focus,
                        target_tree: egui::accesskit::TreeId::ROOT,
                        target_node: id.accesskit_id(),
                        data: None,
                    },
                )],
            );
            frame(&context, vec![]);
            let (_, revealed, clip, text_rect) = frame(&context, vec![]);
            // egui's ScrollArea snaps its content origin to physical pixels. The
            // fractional measured command width can therefore extend by less than
            // half a pixel at the maximum offset; compare its rendered edges using
            // the same pixel grid, while checking its actual glyph bounds exactly.
            let pixels_per_point = context.pixels_per_point();
            let physical_bounds = revealed.round_to_pixels(pixels_per_point);
            let physical_clip = clip.round_to_pixels(pixels_per_point);
            assert!(
                physical_clip.contains_rect(physical_bounds),
                "rendered command {physical_bounds:?}, viewport {physical_clip:?}"
            );
            assert!(clip.contains_rect(text_rect.expect("revealed command text was painted")));
            assert!(clip.contains_rect(revealed.shrink(1.0 / pixels_per_point)));
            assert!(context.memory(|memory| memory.has_focus(id)));

            frame(
                &context,
                vec![
                    egui::Event::PointerMoved(egui::pos2(16.0, 16.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(360.0, 0.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            frame(&context, vec![]);
            let (_, after_manual_scroll, clip, _) = frame(&context, vec![]);
            assert!(after_manual_scroll.left() > clip.right());
            assert!(context.memory(|memory| memory.has_focus(id)));
        }
    }

    #[test]
    fn translated_group_captions_reserve_width_before_narrow_commands() {
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            let context = egui::Context::default();
            crate::configure(&context);
            crate::theme::set_theme(&context, theme);
            context.set_pixels_per_point(1.25);
            for _ in 0..2 {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(900.0, 240.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        ui.horizontal_top(|ui| {
                            group(ui, "Ergebnisse", |ui| {
                                command_menu(
                                    ui,
                                    Icon::Refresh,
                                    "Alles aktualisieren",
                                    false,
                                    CommandSize::Large,
                                );
                            });
                            group(ui, "Abfrageeinrichtung", |ui| {
                                command(ui, Icon::Builder, "Builder", false, CommandSize::Small);
                            });
                        });
                    },
                );
                output.textures_delta.clear();
                let captions: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| {
                        let egui::Shape::Text(text) = &shape.shape else {
                            return None;
                        };
                        ["Ergebnisse", "Abfrageeinrichtung"]
                            .contains(&text.galley.job.text.as_str())
                            .then(|| text.galley.rect.translate(text.pos.to_vec2()))
                    })
                    .collect();
                assert_eq!(captions.len(), 2);
                assert!(
                    (captions[0].top() - captions[1].top()).abs() < 0.1,
                    "command groups must share a caption baseline: {captions:?}"
                );
                assert!(
                    captions[0].right() + GROUP_LABEL_PADDING <= captions[1].left(),
                    "translated captions overlap: {captions:?}"
                );
            }
        }
    }

    #[test]
    fn real_menu_commands_reserve_chevrons_without_relabeling_plain_actions() {
        use egui::accesskit::{HasPopup, Role};
        use egui_kittest::{
            Harness,
            kittest::{NodeT, Queryable},
        };

        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 240.0))
            .build_ui_state(
                |ui, selected: &mut bool| {
                    ui.horizontal_top(|ui| {
                        for (label, size) in [
                            ("Entwurfsansicht wechseln", CommandSize::Large),
                            ("Auswahl", CommandSize::Small),
                        ] {
                            let response = command_menu(ui, Icon::ViewDesign, label, true, size);
                            egui::Popup::menu(&response).show(|ui| {
                                if ui.button("Existing view").clicked() {
                                    *selected = true;
                                    ui.close();
                                }
                            });
                        }
                        command(ui, Icon::Run, "Run", true, CommandSize::Small);
                    });
                },
                false,
            );
        crate::configure(&harness.ctx);
        harness.run();
        for label in ["Entwurfsansicht wechseln", "Auswahl"] {
            let node = harness.get_by_role_and_label(Role::Button, label);
            assert_eq!(node.accesskit_node().has_popup(), Some(HasPopup::Menu));
            assert_eq!(node.accesskit_node().data().is_expanded(), Some(false));
            let rect = node.rect();
            let text = harness
                .output()
                .shapes
                .iter()
                .find_map(|shape| {
                    let egui::Shape::Text(text) = &shape.shape else {
                        return None;
                    };
                    (text.galley.job.text == label)
                        .then(|| text.galley.rect.translate(text.pos.to_vec2()))
                })
                .unwrap();
            let arrow = harness
                .output()
                .shapes
                .iter()
                .find_map(|shape| {
                    let egui::Shape::Path(path) = &shape.shape else {
                        return None;
                    };
                    (path.points.len() == 3
                        && path.points.iter().all(|point| rect.contains(*point)))
                    .then(|| egui::Rect::from_points(&path.points))
                })
                .expect("real menu has a visible chevron inside its hit target");
            assert!(text.bottom() < arrow.top() || text.right() < arrow.left());
        }
        assert_eq!(
            harness.get_by_label("Run").accesskit_node().has_popup(),
            None
        );
        harness.get_by_label("Auswahl").click();
        harness.run();
        assert_eq!(
            harness
                .get_by_label("Auswahl")
                .accesskit_node()
                .data()
                .is_expanded(),
            Some(true)
        );
        harness
            .get_by_role_and_label(Role::Button, "Existing view")
            .click();
        harness.run();
        assert!(*harness.state());
    }

    #[test]
    fn caption_dominant_group_reserves_a_separate_disabled_launcher_rectangle() {
        let context = egui::Context::default();
        context.enable_accesskit();
        crate::configure(&context);
        let caption = "An intentionally long command group caption";
        let frame = || {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 160.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    group_with_launcher(ui, caption, true, |ui| {
                        command(ui, Icon::Cut, "Cut", false, CommandSize::Small);
                    });
                },
            );
            output.textures_delta.clear();
            output
        };
        let _ = frame();
        let output = frame();
        let text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == caption => {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => None,
            })
            .expect("actual group caption paint bounds");
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        let launcher = tree
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.role() == egui::accesskit::Role::Button
                    && node.label().is_some_and(|label| label.starts_with(caption)))
                .then_some(node)
            })
            .expect("actual disabled group launcher");
        let bounds = launcher.bounds().unwrap();
        assert!(
            f64::from(text.right()) + 1.0 <= bounds.x0,
            "caption {text:?} must remain left of launcher {bounds:?}"
        );
        assert!(launcher.is_disabled());
        assert!(!launcher.supports_action(egui::accesskit::Action::Click));
        assert!(!launcher.supports_action(egui::accesskit::Action::Focus));
    }
}
