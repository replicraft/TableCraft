use super::Cache;
use crate::Localizer;
use egui::Ui;
use fluent_bundle::FluentArgs;
use tablecraft_display::{CurrencyChoice, DateFormat, Options, TimeFormat};

const FORMAT_CHOICE_WIDTH: f32 = 280.0;

pub(crate) fn show_options(
    ui: &mut Ui,
    options: &mut Options,
    cache: &Cache,
    localizer: &Localizer,
) {
    ui.add_space(18.0);
    ui.heading(localizer.text("settings-display-formatting"));
    ui.small(localizer.text("settings-format-display-only"));

    let date_label = |choice: DateFormat| {
        example_label(
            localizer.text(date_key(choice)),
            cache.date_example(choice).as_deref(),
        )
    };
    let time_label = |choice: TimeFormat| {
        example_label(
            localizer.text(time_key(choice)),
            cache.time_example(choice).as_deref(),
        )
    };

    let regional_currency = cache.status().and_then(|(_, _, currency)| currency);

    egui::Grid::new("display_format_choices")
        .num_columns(2)
        .spacing(egui::vec2(18.0, 8.0))
        .show(ui, |ui| {
            ui.label(localizer.text("settings-date-format"));
            egui::ComboBox::from_id_salt("display_date_format")
                .width(FORMAT_CHOICE_WIDTH)
                .truncate()
                .selected_text(date_label(options.date))
                .show_ui(ui, |ui| {
                    for choice in DateFormat::ALL {
                        let label = date_label(choice);
                        ui.selectable_value(&mut options.date, choice, &label)
                            .on_hover_text(label);
                    }
                });
            ui.end_row();

            ui.label(localizer.text("settings-time-format"));
            egui::ComboBox::from_id_salt("display_time_format")
                .width(FORMAT_CHOICE_WIDTH)
                .truncate()
                .selected_text(time_label(options.time))
                .show_ui(ui, |ui| {
                    for choice in TimeFormat::ALL {
                        let label = time_label(choice);
                        ui.selectable_value(&mut options.time, choice, &label)
                            .on_hover_text(label);
                    }
                });
            ui.end_row();

            ui.label(localizer.text("settings-currency-format"));
            egui::ComboBox::from_id_salt("display_currency_format")
                .selected_text(currency_label(
                    options.currency,
                    regional_currency,
                    localizer,
                ))
                .show_ui(ui, |ui| {
                    for choice in CurrencyChoice::ALL {
                        ui.selectable_value(
                            &mut options.currency,
                            choice,
                            currency_label(choice, regional_currency, localizer),
                        );
                    }
                });
            ui.end_row();
        });

    if let Some(example) = cache.currency_example() {
        let mut arguments = FluentArgs::new();
        arguments.set("value", example);
        ui.small(localizer.format("settings-currency-example", &arguments));
    }

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

fn currency_label(
    choice: CurrencyChoice,
    regional_currency: Option<&str>,
    localizer: &Localizer,
) -> String {
    match choice {
        CurrencyChoice::SystemLocale => {
            example_label(localizer.text("settings-format-system"), regional_currency)
        }
        CurrencyChoice::Raw => localizer.text("settings-format-raw"),
        explicit => explicit.code().unwrap_or_default().to_owned(),
    }
}

fn example_label(label: String, sample: Option<&str>) -> String {
    match sample {
        Some(sample) => format!("{label} ({sample})"),
        None => label,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cell, Locale};
    use tablecraft_display::{NativeFormat, ScalarPresentation};

    #[test]
    fn system_currency_label_uses_resolved_region_not_translation_or_field_format() {
        let localizer = Localizer::new(Locale::Ja).unwrap();
        let mut cache = Cache::default();

        for (locale, expected) in [("en-US", "USD"), ("de-DE", "EUR"), ("ja-JP", "JPY")] {
            cache.update(&Options {
                system_locale: Some(locale.into()),
                ..Options::default()
            });
            let regional_currency = cache.status().unwrap().2;
            assert_eq!(regional_currency, Some(expected));
            assert_eq!(
                currency_label(CurrencyChoice::SystemLocale, regional_currency, &localizer),
                format!("{} ({expected})", localizer.text("settings-format-system"))
            );
        }

        cache.update(&Options {
            system_locale: Some("en-US".into()),
            ..Options::default()
        });
        let mut euro_field = Cell::scalar("raw");
        euro_field.presentation = Some(ScalarPresentation::Currency {
            scaled: 100_000,
            native_format: NativeFormat::Euro,
        });
        cache.apply(&mut euro_field);
        assert!(euro_field.text.contains('€'));
        assert_eq!(cache.status().unwrap().2, Some("USD"));
        assert_eq!(
            currency_label(
                CurrencyChoice::SystemLocale,
                cache.status().unwrap().2,
                &localizer
            ),
            format!("{} (USD)", localizer.text("settings-format-system"))
        );

        assert_eq!(
            currency_label(CurrencyChoice::SystemLocale, None, &localizer),
            localizer.text("settings-format-system")
        );
        assert_eq!(
            currency_label(CurrencyChoice::Eur, Some("USD"), &localizer),
            "EUR"
        );
        assert_eq!(
            currency_label(CurrencyChoice::Raw, Some("USD"), &localizer),
            localizer.text("settings-format-raw")
        );
    }
}
