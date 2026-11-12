//! Native TableCraft workspace. Database work is isolated from the event thread.
mod diagnostics;
mod file_dialog;
mod font_inventory;
mod mock;
mod query_formats;
mod regional;
mod updating;
mod view_models;
mod workspace;

use clap::Parser;
use std::{error::Error, path::PathBuf};
use tablecraft_runtime::QueryExpressionFormats;
use tablecraft_ui::{Locale, Localizer, Theme, preview::Scene};

/// Open a native Access database in the desktop workspace.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Emit payload-free operation diagnostics to stderr; repeat for I/O detail.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Optional MDB/ACCDB path to open after startup.
    #[arg(conflicts_with = "mock")]
    database: Option<PathBuf>,

    /// Numeric Text conversion/output profile: invariant, en-us or de-de; unset stays unavailable.
    #[arg(long, conflicts_with = "mock", value_name = "PROFILE", value_parser = query_formats::number)]
    query_number_format: Option<QueryExpressionFormats>,

    /// Gregorian date Text conversion/output profile: iso, en-us or de-de; unset stays unavailable.
    #[arg(long, conflicts_with = "mock", value_name = "PROFILE", value_parser = query_formats::date)]
    query_date_format: Option<QueryExpressionFormats>,

    /// Review the interface with synthetic data and no database services.
    #[arg(long)]
    mock: bool,

    /// Initial synthetic view (datasheet, design, query, saved-query, etc.).
    #[arg(long, requires = "mock", default_value = "datasheet")]
    mock_view: Scene,

    /// Initial preview language; the Japanese alias jp is accepted.
    #[arg(long, requires = "mock")]
    mock_language: Option<Locale>,

    /// Start the synthetic preview in the dark theme.
    #[arg(long, requires = "mock")]
    mock_dark: bool,
}

fn main() -> eframe::Result {
    let args = Args::parse();
    diagnostics::init(args.verbose).map_err(eframe::Error::AppCreation)?;
    let options = eframe::NativeOptions {
        viewport: desktop_viewport()
            .map_err(|error| eframe::Error::AppCreation(Box::new(error)))?,
        ..Default::default()
    };
    eframe::run_native(
        "TableCraft",
        options,
        Box::new(move |cc| {
            tablecraft_ui::configure(&cc.egui_ctx);
            if args.mock {
                let localizer = Localizer::new(args.mock_language.unwrap_or_default())
                    .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?;
                let theme = if args.mock_dark {
                    Theme::Dark
                } else {
                    Theme::System
                };
                Ok(Box::new(mock::MockApp::new(
                    args.mock_view,
                    localizer,
                    theme,
                )))
            } else {
                // NO_CLONE: eframe owns CreationContext; the desktop read worker moves an owned
                // repaint Context into its long-lived callback. Context shares egui state
                // rather than copying the UI.
                let desktop = match (args.query_number_format, args.query_date_format) {
                    (None, None) => workspace::Desktop::new(cc.egui_ctx.clone(), args.database),
                    (number, date) => workspace::Desktop::with_query_formats(
                        cc.egui_ctx.clone(),
                        args.database,
                        QueryExpressionFormats {
                            number: number.and_then(|formats| formats.number),
                            date: date.and_then(|formats| formats.date),
                        },
                    ),
                };
                Ok(Box::new(desktop))
            }
        }),
    )
}

fn desktop_viewport() -> Result<egui::ViewportBuilder, tablecraft_ui::ApplicationIconError> {
    Ok(egui::ViewportBuilder::default()
        .with_icon(tablecraft_ui::application_icon()?)
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([900.0, 580.0])
        .with_decorations(!cfg!(any(target_os = "windows", target_os = "linux"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_query_formats_use_owner_profiles_and_remain_independent() {
        let empty = Args::try_parse_from(["tablecraft-desktop"]).unwrap();
        assert!(empty.query_number_format.is_none());
        assert!(empty.query_date_format.is_none());
        let parsed = Args::try_parse_from([
            "tablecraft-desktop",
            "--query-number-format",
            "DE-DE",
            "--query-date-format",
            "iso",
        ])
        .unwrap();
        assert_eq!(
            parsed.query_number_format,
            Some(QueryExpressionFormats::from_profile_names(Some("de-de"), None).unwrap())
        );
        assert_eq!(
            parsed.query_date_format,
            Some(QueryExpressionFormats::from_profile_names(None, Some("iso")).unwrap())
        );
    }

    #[test]
    fn invalid_or_synthetic_query_format_configuration_never_silently_falls_back() {
        for arguments in [
            vec!["tablecraft-desktop", "--query-number-format", "system"],
            vec!["tablecraft-desktop", "--query-date-format", "fr-fr"],
            vec![
                "tablecraft-desktop",
                "--query-number-format",
                "de-de",
                "--mock",
            ],
        ] {
            assert!(Args::try_parse_from(arguments).is_err());
        }
    }

    #[test]
    fn desktop_decorations_follow_platform_ownership() {
        let viewport = desktop_viewport().unwrap();
        assert_eq!(
            viewport.decorations,
            Some(!cfg!(any(target_os = "windows", target_os = "linux")))
        );
        assert_eq!(viewport.min_inner_size, Some(egui::vec2(900.0, 580.0)));
        let icon = viewport.icon.as_ref().expect("bundled application icon");
        assert_eq!((icon.width, icon.height), (32, 32));
        assert_eq!(icon.rgba.len(), 32 * 32 * 4);
        assert!(
            icon.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == 0)
        );
        assert!(
            icon.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] > pixel[1] && pixel[3] == 255)
        );
    }

    #[test]
    fn mock_mode_requires_explicit_selection_and_cannot_open_a_database_path() {
        let ordinary = Args::try_parse_from(["tablecraft-desktop"]).unwrap();
        assert!(!ordinary.mock);
        assert!(ordinary.database.is_none());

        let preview = Args::try_parse_from([
            "tablecraft-desktop",
            "--mock",
            "--mock-view",
            "design",
            "--mock-language",
            "jp",
            "--mock-dark",
        ])
        .unwrap();
        assert!(preview.mock);
        assert_eq!(preview.mock_view, Scene::Design);
        assert_eq!(preview.mock_language, Some(Locale::Ja));
        assert!(preview.mock_dark);

        assert!(Args::try_parse_from(["tablecraft-desktop", "sample.accdb", "--mock",]).is_err());
        assert!(Args::try_parse_from(["tablecraft-desktop", "--mock-language", "ja",]).is_err());
        assert!(
            Args::try_parse_from(["tablecraft-desktop", "--mock", "--mock-view", "not-a-view",])
                .is_err()
        );
    }
}
