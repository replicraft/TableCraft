//! Rasterize the original application vector for native packaging, without a window.
//!
//! Set TABLECRAFT_ICON_DIR to a fresh directory whose parent already exists.
//! This renders each resolution directly; it never enlarges a 32-pixel raster.
#[path = "export_application_icons/formats.rs"]
mod formats;

#[cfg(test)]
#[path = "export_application_icons/tests.rs"]
mod tests;

use egui::load::SizeHint;
use formats::PngIcon;
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

const SOURCE: &[u8] =
    include_bytes!("../../../../assets/ui/icons/tablecraft/application-light.svg");
const PNG_SIZES: [u32; 9] = [16, 24, 32, 48, 64, 128, 256, 512, 1024];

fn main() -> Result<(), Box<dyn Error>> {
    let directory = env::var_os("TABLECRAFT_ICON_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "TABLECRAFT_ICON_DIR must name an explicit fresh output directory",
            )
        })?;

    export(&directory)?;

    println!("format\tfile\nICO\tapplication.ico\nICNS\tapplication.icns");
    for size in PNG_SIZES {
        println!("PNG\tapplication-{size}.png");
    }

    Ok(())
}

fn rasterize(size: u32) -> Result<PngIcon, Box<dyn Error>> {
    let pixels = egui_extras::image::load_svg_bytes_with_size(
        SOURCE,
        SizeHint::Size {
            width: size,
            height: size,
            maintain_aspect_ratio: true,
        },
        &Default::default(),
    )
    .map_err(io::Error::other)?;
    if pixels.size != [size as usize, size as usize] {
        return Err(io::Error::other("SVG renderer returned unexpected dimensions").into());
    }

    // Egui's raster pixels are premultiplied. PNG stores straight RGBA, including
    // antialiased transparent edges; copying raw Color32 bytes would darken them.
    let rgba = pixels
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect::<Vec<_>>();
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(&rgba, size, size, ColorType::Rgba8.into())?;
    Ok(PngIcon { size, bytes })
}

fn export(directory: &Path) -> Result<(), Box<dyn Error>> {
    // Finish all rendering/encoding before publishing any files. create_dir and
    // create_new reject replacement, including an already existing empty folder.
    let images = PNG_SIZES
        .into_iter()
        .map(rasterize)
        .collect::<Result<Vec<_>, _>>()?;
    let ico = formats::ico(&images)?;
    let icns = formats::icns(&images)?;

    fs::create_dir(directory)?;
    for image in &images {
        write_new(
            &directory.join(format!("application-{}.png", image.size)),
            &image.bytes,
        )?;
    }
    write_new(&directory.join("application.ico"), &ico)?;
    write_new(&directory.join("application.icns"), &icns)?;

    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(path)?;

    file.write_all(bytes)?;
    file.flush()
}
