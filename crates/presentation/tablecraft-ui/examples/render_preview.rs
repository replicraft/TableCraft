//! Export deterministic shared UI frames, without opening an OS window or database.
use egui_kittest::{Harness, kittest::Queryable};
use image::ImageFormat;
use std::{env, error::Error, fs, io, path::PathBuf};
use tablecraft_ui::{
    Locale, Localizer, Theme,
    preview::{Preview, Scene},
};

const ANIMATED_SCENE_STEPS: usize = 3;
const LAYOUT_SETTLE_STEPS: usize = 2;
const MIN_SCALE: f32 = 1.0;
const MAX_SCALE: f32 = 2.0;
const MAX_F6_STEPS: u8 = 5;

fn main() -> Result<(), Box<dyn Error>> {
    let directory = env::var_os("TABLECRAFT_PREVIEW_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".cache/ui-previews"));
    let locale = setting("TABLECRAFT_PREVIEW_LOCALE", "en-US")?.parse::<Locale>()?;
    let regional_locale = setting("TABLECRAFT_PREVIEW_REGIONAL_LOCALE", "en-US")?;
    regional_locale.parse::<icu_locale_core::Locale>()?;
    let theme_name = setting("TABLECRAFT_PREVIEW_THEME", "light")?;
    let theme = match theme_name.as_str() {
        "light" => Theme::Light,
        "dark" => Theme::Dark,
        _ => return Err(io::Error::other("preview theme must be light or dark").into()),
    };
    let scenes = match env::var("TABLECRAFT_PREVIEW_SCENE") {
        Ok(scene) => vec![scene.parse::<Scene>()?],
        Err(env::VarError::NotPresent) => Scene::ALL.to_vec(),
        Err(error) => return Err(error.into()),
    };
    let host = setting("TABLECRAFT_PREVIEW_HOST", "synthetic")?;
    if !matches!(host.as_str(), "synthetic" | "normal") {
        return Err(io::Error::other("preview host must be synthetic or normal").into());
    }
    let font_search = match env::var("TABLECRAFT_PREVIEW_FONT_SEARCH") {
        Ok(query) => Some(query),
        Err(env::VarError::NotPresent) => None,
        Err(error) => return Err(error.into()),
    };
    if font_search.is_some() && scenes != [Scene::Datasheet] {
        return Err(io::Error::other("font-search rendering requires the datasheet scene").into());
    }
    let search_suffix = if font_search.is_some() {
        "-font-search"
    } else {
        ""
    };
    let width = dimension("TABLECRAFT_PREVIEW_WIDTH", "1280")?;
    let height = dimension("TABLECRAFT_PREVIEW_HEIGHT", "820")?;
    let scale = setting("TABLECRAFT_PREVIEW_SCALE", "1")?.parse::<f32>()?;
    if !(MIN_SCALE..=MAX_SCALE).contains(&scale) {
        return Err(io::Error::other("preview scale must be finite and between 1 and 2").into());
    }
    let scale_suffix = if scale == MIN_SCALE {
        String::new()
    } else {
        format!("@{scale}x")
    };
    let focus_steps = setting("TABLECRAFT_PREVIEW_F6_STEPS", "0")?.parse::<u8>()?;
    if focus_steps > MAX_F6_STEPS {
        return Err(io::Error::other("preview F6 steps must be between 0 and 5").into());
    }
    let ribbon_focus = setting("TABLECRAFT_PREVIEW_RIBBON_FOCUS", "none")?;
    let ribbon_suffix = match ribbon_focus.as_str() {
        "none" => "",
        "tab" => "-ribbon-tab",
        "commands" => "-ribbon-commands",
        _ => {
            return Err(
                io::Error::other("preview Ribbon focus must be none, tab or commands").into(),
            );
        }
    };
    if !ribbon_suffix.is_empty() && (focus_steps != 0 || font_search.is_some()) {
        return Err(io::Error::other("choose one preview keyboard focus workflow").into());
    }
    let focus_suffix = if focus_steps == 0 {
        String::new()
    } else {
        format!("-f6-{focus_steps}")
    };
    fs::create_dir_all(&directory)?;

    let mut preview = Preview::new(Scene::Datasheet);
    preview.state_mut().localizer = Localizer::new(locale)?;
    preview.state_mut().theme = theme;
    preview.state_mut().formatting.system_locale = Some(regional_locale);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(width, height))
        .with_pixels_per_point(scale)
        .with_os(egui::os::OperatingSystem::Windows)
        .with_max_steps(32)
        .build_ui_state(
            |ui, preview: &mut Preview| {
                // kittest adds an eight-point demonstration frame around UiState.
                // The real desktop receives a full-viewport root Ui without it.
                let viewport = ui.ctx().viewport_rect();
                let mut root = egui::Ui::new(
                    ui.ctx().clone(),
                    egui::Id::new("tablecraft_preview_root"),
                    egui::UiBuilder::new()
                        .layer_id(egui::LayerId::background())
                        .max_rect(viewport),
                );
                root.set_clip_rect(viewport);
                preview.show(&mut root);
            },
            preview,
        );
    tablecraft_ui::configure(&harness.ctx);
    assert_eq!(
        harness.ctx.viewport_rect().size(),
        egui::vec2(width, height)
    );
    assert_eq!(harness.ctx.pixels_per_point(), scale);

    for scene in scenes {
        harness.state_mut().set_scene(scene);
        // Retain synthetic data and the resource-free host while exercising
        // normal presentation gates explicitly.
        harness.state_mut().state_mut().mock_preview = host == "synthetic";
        // Layout captures should not acquire a tooltip from the harness cursor.
        // Interaction tests choose their pointer position explicitly.
        harness.remove_cursor();
        if scene.is_animated() {
            // Lifecycle spinners deliberately request repaints while pending.
            // Advance a fixed number of frames instead of waiting for idle.
            harness.run_steps(ANIMATED_SCENE_STEPS);
        } else {
            // A changed scene can alter an auto-sized footer's previous-frame
            // height. Capture the settled layout, as interaction tests do.
            harness.run_steps(LAYOUT_SETTLE_STEPS);
            harness.run();
        }
        for _ in 0..focus_steps {
            harness.key_press(egui::Key::F6);
            if scene.is_animated() {
                harness.run_steps(ANIMATED_SCENE_STEPS);
            } else {
                harness.run();
            }
        }
        if !ribbon_suffix.is_empty() {
            harness.key_press(egui::Key::F10);
            harness.run_steps(LAYOUT_SETTLE_STEPS);
            if ribbon_focus == "commands" {
                harness.key_press(egui::Key::ArrowDown);
                harness.run_steps(LAYOUT_SETTLE_STEPS);
            }
        }
        if let Some(query) = &font_search {
            let label = harness.state().state().localizer.text("ribbon-font");
            harness
                .get_by_role_and_label(egui::accesskit::Role::ComboBox, &label)
                .click_accesskit();
            harness.run_steps(LAYOUT_SETTLE_STEPS);
            harness.event(egui::Event::Text(query.clone()));
            harness.run_steps(LAYOUT_SETTLE_STEPS);
        }
        let image = harness.render().map_err(io::Error::other)?;
        let path = directory.join(format!(
            "{}-{}-{}{}{}{ribbon_suffix}{}.png",
            locale.as_str(),
            theme_name,
            scene.as_str(),
            focus_suffix,
            search_suffix,
            scale_suffix,
        ));
        image.save_with_format(&path, ImageFormat::Png)?;
        println!("{}", path.display());
    }
    Ok(())
}

fn setting(name: &str, default: &str) -> Result<String, env::VarError> {
    match env::var(name) {
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Ok(default.into()),
        Err(error) => Err(error),
    }
}

fn dimension(name: &str, default: &str) -> Result<f32, Box<dyn Error>> {
    let value = setting(name, default)?.parse::<u16>()?;
    if !(320..=3840).contains(&value) {
        return Err(io::Error::other("preview dimensions must be between 320 and 3840").into());
    }
    Ok(f32::from(value))
}
