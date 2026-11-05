use super::Cache;
use crate::Localizer;
use egui::Ui;
use tablecraft_display::{CurrencyChoice, DateFormat, Options, TimeFormat};

pub(crate) fn show_options(
    ui: &mut Ui,
    options: &mut Options,
    cache: &Cache,
    localizer: &Localizer,
) {
    ui.add_space(18.0);
    ui.heading(localizer.text("settings-display-formatting"));
    ui.small(localizer.text("settings-format-display-only"));

    let examples = cache.system_examples();
    let date_label = |choice: DateFormat| {
        example_label(
            localizer.text(date_key(choice)),
            choice == DateFormat::SystemLocale,
            examples.as_ref().map(|pair| pair.0.as_str()),
        )
    };
    let time_label = |choice: TimeFormat| {
        example_label(
            localizer.text(time_key(choice)),
            choice == TimeFormat::SystemLocale,
            examples.as_ref().map(|pair| pair.1.as_str()),
        )
    };

    egui::Grid::new("display_format_choices")
        .num_columns(2)
        .spacing(egui::vec2(18.0, 8.0))
        .show(ui, |ui| {
            ui.label(localizer.text("settings-date-format"));
            egui::ComboBox::from_id_salt("display_date_format")
                .selected_text(date_label(options.date))
                .show_ui(ui, |ui| {
                    for choice in DateFormat::ALL {
                        ui.selectable_value(&mut options.date, choice, date_label(choice));
                    }
                });
            ui.end_row();

            ui.label(localizer.text("settings-time-format"));
            egui::ComboBox::from_id_salt("display_time_format")
                .selected_text(time_label(options.time))
                .show_ui(ui, |ui| {
                    for choice in TimeFormat::ALL {
                        ui.selectable_value(&mut options.time, choice, time_label(choice));
                    }
                });
            ui.end_row();

            ui.label(localizer.text("settings-currency-format"));
            egui::ComboBox::from_id_salt("display_currency_format")
                .selected_text(currency_label(options.currency, localizer))
                .show_ui(ui, |ui| {
                    for choice in CurrencyChoice::ALL {
                        ui.selectable_value(
                            &mut options.currency,
                            choice,
                            currency_label(choice, localizer),
                        );
                    }
                });
            ui.end_row();
        });

    if let Some((locale, fallback, currency)) = cache.status() {
        ui.horizontal_wrapped(|ui| {
            ui.label(localizer.text("settings-regional-locale"));
            ui.label(locale);
            if let Some(code) = currency {
                ui.label(code);
            }
        });
        if fallback {
            ui.small(localizer.text("settings-locale-fallback"));
        }
        if currency.is_none() {
            ui.small(localizer.text("settings-currency-unresolved"));
        }
    } else {
        ui.label(localizer.text("settings-format-unavailable"));
    }

    ui.small(localizer.text("settings-format-system-help"));
    ui.small(localizer.text("settings-currency-unit-help"));
    ui.small(localizer.text("settings-format-raw-help"));
}

fn currency_label(choice: CurrencyChoice, localizer: &Localizer) -> String {
    match choice {
        CurrencyChoice::SystemLocale => localizer.text("settings-format-system"),
        CurrencyChoice::Raw => localizer.text("settings-format-raw"),
        explicit => explicit.code().unwrap_or_default().to_owned(),
    }
}

fn example_label(label: String, system: bool, sample: Option<&str>) -> String {
    match (system, sample) {
        (true, Some(sample)) => format!("{label} ({sample})"),
        _ => label,
    }
}

fn date_key(choice: DateFormat) -> &'static str {
    match choice {
        DateFormat::SystemLocale => "settings-format-system",
        DateFormat::Iso => "settings-date-iso",
        DateFormat::DayMonthYear => "settings-date-dmy",
        DateFormat::MonthDayYear => "settings-date-mdy",
        DateFormat::Short => "settings-format-short",
        DateFormat::Long => "settings-format-long",
        DateFormat::Raw => "settings-format-raw",
    }
}

fn time_key(choice: TimeFormat) -> &'static str {
    match choice {
        TimeFormat::SystemLocale => "settings-format-system",
        TimeFormat::HoursMinutes => "settings-time-hm",
        TimeFormat::HoursMinutesSeconds => "settings-time-hms",
        TimeFormat::TwelveHour => "settings-time-twelve",
        TimeFormat::Fractional => "settings-time-fractional",
        TimeFormat::Raw => "settings-format-raw",
    }
}
