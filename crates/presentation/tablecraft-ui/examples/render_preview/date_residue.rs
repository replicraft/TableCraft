//! A fixed, independent DATE residue input for actual datasheet rendering.
use egui::{Event, Modifiers, PointerButton, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use jiff::civil::DateTime;
use tablecraft_display::{
    DateFormat, DateRepresentation, NativeFormat, ScalarPresentation, TimeFormat,
};
use tablecraft_ui::preview::Preview;

const REGISTERED_FIELD: &str = "Registered";
const WHOLE_SECOND_BITS: u64 = 0x40e6_9fb1_a666_6666;
const EXTRA_COLUMN_WIDTH: f32 = 180.0;

pub(super) fn apply(preview: &mut Preview, mode: &str) {
    let state = preview.state_mut();
    state.formatting.date = DateFormat::Iso;
    state.formatting.time = match mode {
        "seconds" => TimeFormat::HoursMinutesSeconds,
        "fractional" => TimeFormat::Fractional,
        "raw" => TimeFormat::Raw,
        _ => unreachable!("the renderer validates its explicit mode"),
    };
    let data = state.data.as_mut().expect("datasheet preview data");
    let column = data
        .columns
        .iter()
        .position(|column| column.name == REGISTERED_FIELD)
        .expect("the datasheet fixture has a registered-date column");
    let cell = &mut data.rows[0][column];
    // Independent IEEE DATE golden for 2026-11-07T13:14:15. The decoded
    // nanoseconds retain the double's residue instead of hiding it in the input.
    cell.presentation = Some(ScalarPresentation::DateTime {
        calendar: DateTime::constant(2026, 11, 7, 13, 14, 14, 999_999_749),
        representation: DateRepresentation::LegacyBits(WHOLE_SECOND_BITS),
        native_format: NativeFormat::GeneralDate,
    });
    cell.text = "DATE(0x40e69fb1a6666666)".into();
}

/// Use the real column divider so fractional/raw output is visible in the frame.
pub(super) fn reveal(harness: &mut Harness<'_, Preview>) {
    let column = harness
        .state()
        .state()
        .data
        .as_ref()
        .expect("datasheet preview data")
        .columns
        .iter()
        .position(|column| column.name == REGISTERED_FIELD)
        .expect("the datasheet fixture has a registered-date column");
    // Header containers expose their column position, while the field name is
    // a child label. The record selector is a RowHeader, not a data header.
    let header = harness
        .get_all_by_role(Role::ColumnHeader)
        .nth(column)
        .expect("the actual date column header")
        .rect();
    let next = harness
        .get_all_by_role(Role::ColumnHeader)
        .nth(column + 1)
        .expect("the following column header")
        .rect();
    let start = egui::pos2(next.left(), header.center().y);
    harness.event(Event::PointerMoved(start));
    harness.run();
    harness.event(Event::PointerButton {
        pos: start,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    let end = start + egui::vec2(EXTRA_COLUMN_WIDTH, 0.0);
    harness.event(Event::PointerMoved(end));
    harness.run();
    harness.event(Event::PointerButton {
        pos: end,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    assert!(
        harness
            .get_all_by_role(Role::ColumnHeader)
            .nth(column)
            .expect("the resized date column header")
            .rect()
            .width()
            > header.width() + EXTRA_COLUMN_WIDTH / 2.0,
        "the actual date column must be wide enough to inspect its output: initial {header:?}"
    );
    harness.remove_cursor();
    harness.run();
}
