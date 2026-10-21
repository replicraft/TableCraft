//! Fixed German standalone calendar labels; no ambient locale or prefix guessing.
use jiff::civil::Weekday;

const MONTHS: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];
const SHORT_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
];

pub(super) fn month(month: u8, abbreviated: bool) -> &'static str {
    let names = if abbreviated { &SHORT_MONTHS } else { &MONTHS };
    names[usize::from(month - 1)]
}
pub(super) fn weekday(day: Weekday, abbreviated: bool) -> &'static str {
    match (day, abbreviated) {
        (Weekday::Monday, false) => "Montag",
        (Weekday::Monday, true) => "Mo",
        (Weekday::Tuesday, false) => "Dienstag",
        (Weekday::Tuesday, true) => "Di",
        (Weekday::Wednesday, false) => "Mittwoch",
        (Weekday::Wednesday, true) => "Mi",
        (Weekday::Thursday, false) => "Donnerstag",
        (Weekday::Thursday, true) => "Do",
        (Weekday::Friday, false) => "Freitag",
        (Weekday::Friday, true) => "Fr",
        (Weekday::Saturday, false) => "Samstag",
        (Weekday::Saturday, true) => "Sa",
        (Weekday::Sunday, false) => "Sonntag",
        (Weekday::Sunday, true) => "So",
    }
}
