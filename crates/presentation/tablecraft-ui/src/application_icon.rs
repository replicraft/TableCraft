//! Bundled original application artwork for native window and task-switcher icons.

#[derive(Debug, thiserror::Error)]
#[error("the bundled application icon could not be decoded: {0}")]
pub struct ApplicationIconError(String);

pub fn application_icon() -> Result<egui::IconData, ApplicationIconError> {
    let image = egui_extras::image::load_svg_bytes(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/ui/icons/tablecraft/application-light.svg"
        )),
        &Default::default(),
    )
    .map_err(ApplicationIconError)?;
    let width =
        u32::try_from(image.size[0]).map_err(|error| ApplicationIconError(error.to_string()))?;
    let height =
        u32::try_from(image.size[1]).map_err(|error| ApplicationIconError(error.to_string()))?;
    let rgba = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect();
    Ok(egui::IconData {
        rgba,
        width,
        height,
    })
}
