//! Native TableCraft workspace. Database work is isolated from the event thread.
mod diagnostics;
mod file_dialog;
mod workspace;

use clap::Parser;
use std::path::PathBuf;

/// Open a native Access database in the desktop workspace.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Emit payload-free operation diagnostics to stderr; repeat for I/O detail.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Optional MDB/ACCDB path to open after startup.
    database: Option<PathBuf>,
}

fn main() -> eframe::Result {
    let args = Args::parse();
    diagnostics::init(args.verbose).map_err(eframe::Error::AppCreation)?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 580.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TableCraft",
        options,
        Box::new(move |cc| {
            tablecraft_ui::configure(&cc.egui_ctx);
            Ok(Box::new(workspace::Desktop::new(
                cc.egui_ctx.clone(),
                args.database,
            )))
        }),
    )
}
