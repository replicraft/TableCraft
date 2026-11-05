//! Export deterministic shared UI frames, without opening an OS window or database.
use egui_kittest::Harness;
use image::ImageFormat;
use std::{env, error::Error, fs, io, path::PathBuf};
use tablecraft_ui::{
    Locale, Localizer, Theme,
    preview::{Preview, Scene},
};

const ANIMATED_SCENE_STEPS: usize = 3;

fn main() -> Result<(), Box<dyn Error>> {
    let directory = env::var_os("TABLECRAFT_PREVIEW_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".cache/ui-previews"));
    let locale = setting("TABLECRAFT_PREVIEW_LOCALE", "en-US")?.parse::<Locale>()?;
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
    let width = dimension("TABLECRAFT_PREVIEW_WIDTH", "1280")?;
    let height = dimension("TABLECRAFT_PREVIEW_HEIGHT", "820")?;
    fs::create_dir_all(&directory)?;

    let mut preview = Preview::new(Scene::Datasheet);
    preview.state_mut().localizer = Localizer::new(locale)?;
    preview.state_mut().theme = theme;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(width, height))
        .with_pixels_per_point(1.0)
        .with_os(egui::os::OperatingSystem::Windows)
        .with_max_steps(32)
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    tablecraft_ui::configure(&harness.ctx);

    for scene in scenes {
        harness.state_mut().set_scene(scene);
        if scene == Scene::Loading {
            // A busy spinner deliberately requests another repaint forever.
            // Advance a fixed number of frames instead of waiting for idle.
            harness.run_steps(ANIMATED_SCENE_STEPS);
        } else {
            harness.run();
        }
        let image = harness.render().map_err(io::Error::other)?;
        let path = directory.join(format!(
            "{}-{}-{}.png",
            locale.as_str(),
            theme_name,
            scene.as_str()
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
