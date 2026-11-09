use super::super::{Locale, LocalizationDiagnostics, Localizer, install, message};
use egui::{Context, RawInput};
use fluent_bundle::FluentArgs;

struct Borrowed<'a>(&'a str);

impl AsRef<str> for Borrowed<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

fn formatted<S: AsRef<str>>(localizer: &Localizer, id: &str, values: &[(&str, S)]) -> String {
    let context = Context::default();
    install(&context, localizer);
    let mut text = String::new();
    let mut output = context.run_ui(RawInput::default(), |ui| {
        text = message(ui, id, values);
    });
    output.textures_delta.clear();
    text
}

#[test]
fn borrowed_and_owned_arguments_preserve_all_locales_and_literal_cell_content() {
    let column = String::from("TrueName · { $column } · 名前");
    let text = String::from("NULL is text · { $row } · <tag> & \"quoted\"");
    let context = String::from("Stored result; no expression evaluated");
    let ownership = (
        column.as_ptr(),
        column.capacity(),
        text.as_ptr(),
        text.capacity(),
    );
    for locale in Locale::ALL {
        let localizer = Localizer::new(locale).unwrap();
        let mut arguments = FluentArgs::new();
        arguments.set("column", column.as_str());
        arguments.set("row", "42");
        arguments.set("text", text.as_str());
        arguments.set("context", context.as_str());
        let expected = localizer.format("datasheet-cell-details", &arguments);
        let borrowed = formatted(
            &localizer,
            "datasheet-cell-details",
            &[
                ("column", Borrowed(column.as_str())),
                ("row", Borrowed("42")),
                ("text", Borrowed(text.as_str())),
                ("context", Borrowed(context.as_str())),
            ],
        );
        let owned = formatted(
            &localizer,
            "datasheet-cell-details",
            &[
                ("column", column.clone()),
                ("row", "42".to_owned()),
                ("text", text.clone()),
                ("context", context.clone()),
            ],
        );
        assert_eq!(borrowed, expected, "{locale:?}");
        assert_eq!(owned, expected, "{locale:?}");
        assert!(borrowed.contains(column.as_str()), "{locale:?}");
        assert!(borrowed.contains(text.as_str()), "{locale:?}");
        assert_eq!(localizer.diagnostics(), LocalizationDiagnostics::default());
    }
    assert_eq!(
        ownership,
        (
            column.as_ptr(),
            column.capacity(),
            text.as_ptr(),
            text.capacity()
        )
    );
}

#[test]
fn borrowed_headers_and_empty_text_keep_the_original_field_and_row_arguments() {
    for locale in Locale::ALL {
        let localizer = Localizer::new(locale).unwrap();
        let mut arguments = FluentArgs::new();
        arguments.set("name", "StoredName");
        assert_eq!(
            formatted(
                &localizer,
                "datasheet-field-name",
                &[("name", "StoredName")]
            ),
            localizer.format("datasheet-field-name", &arguments),
            "{locale:?}"
        );
        for text in ["", "NULL"] {
            let mut arguments = FluentArgs::new();
            arguments.set("column", "StoredName");
            arguments.set("row", "7");
            arguments.set("text", text);
            assert_eq!(
                formatted(
                    &localizer,
                    "datasheet-cell-context",
                    &[("column", "StoredName"), ("row", "7"), ("text", text)],
                ),
                localizer.format("datasheet-cell-context", &arguments),
                "{locale:?}/{text:?}"
            );
        }
        assert_eq!(localizer.diagnostics(), LocalizationDiagnostics::default());
    }
}

#[test]
fn borrowed_argument_failures_keep_safe_fallback_and_diagnostics() {
    for locale in Locale::ALL {
        let localizer = Localizer::new(locale).unwrap();
        let expected = localizer.text("shell-translation-unavailable");
        assert_eq!(
            formatted::<&str>(&localizer, "unknown-message", &[]),
            expected
        );
        assert!(localizer.diagnostics().missing_messages > 0);
        assert_eq!(
            formatted::<&str>(&localizer, "datasheet-column-count", &[]),
            expected
        );
        assert!(localizer.diagnostics().formatting_errors > 0);
        assert_eq!(
            formatted::<&str>(&localizer, "shell-ready", &[]),
            localizer.text("shell-ready")
        );
    }
}
