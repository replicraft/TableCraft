use super::*;
use jiff::civil::date;
use tablecraft_display::{CurrencyChoice, DateRepresentation, NativeFormat, ScalarPresentation};

fn options(locale: &str) -> Options {
    Options {
        system_locale: Some(locale.to_owned()),
        ..Options::default()
    }
}

fn money(scaled: i64) -> ScalarPresentation {
    ScalarPresentation::Currency {
        scaled,
        native_format: NativeFormat::Currency,
    }
}

#[test]
fn policy_changes_and_new_publications_format_typed_cells_and_keep_copy_text_literal() {
    use crate::{Cell, Data, State};

    let mut state = State::default();
    state.dismiss_backstage();
    state.formatting = options("en-US");
    state.formatting.currency = CurrencyChoice::Jpy;
    let mut typed = Cell::scalar("old host display text");
    typed.presentation = Some(money(12_345_678));
    let page = |typed| Data {
        query_contents: None,
        row_ordinals: None,
        source_revision: Some(1),
        table: "Items".into(),
        offset: 0,
        columns: vec!["Amount".into(), "Literal".into()],
        rows: vec![vec![typed, Cell::scalar("$1,234.57")]],
        has_more: false,
    };
    state.set_table_data(page(typed));
    assert_eq!(state.selected_cell_text(), Some("¥1,235"));
    assert_eq!(state.data.as_ref().unwrap().rows[0][1].text, "$1,234.57");

    state.formatting.currency = CurrencyChoice::Raw;
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        state.show(ui);
    });
    output.textures_delta.clear();
    assert_eq!(state.selected_cell_text(), Some("1234.5678"));

    let mut replacement = Cell::scalar("new host display text");
    replacement.presentation = Some(money(1));
    state.set_table_data(page(replacement));
    assert_eq!(state.selected_cell_text(), Some("0.0001"));
    assert_eq!(state.data.as_ref().unwrap().rows[0][1].text, "$1,234.57");
}

#[test]
fn unavailable_formatter_cannot_retain_stale_currency_or_date_display() {
    use crate::Cell;

    let cache = Cache {
        policy: Some(options("en-US")),
        engine: None,
    };
    let mut currency = Cell::scalar("¥1,235");
    currency.presentation = Some(money(12_345_678));
    cache.apply(&mut currency);
    assert_eq!(currency.text, "1234.5678");

    let mut date_cell = Cell::scalar("4/12/2026");
    date_cell.presentation = Some(ScalarPresentation::DateTime {
        calendar: date(2026, 4, 12).at(15, 4, 5, 123_456_700),
        representation: DateRepresentation::Extended,
        native_format: NativeFormat::ShortDate,
    });
    cache.apply(&mut date_cell);
    assert_eq!(date_cell.text, "DATETIME2(2026-04-12T15:04:05.1234567)");
    assert!(cache.status().is_none());
}
