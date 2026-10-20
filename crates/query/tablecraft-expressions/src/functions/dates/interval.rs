//! Shared interval identities; each operation defines its own weekday semantics.
#[derive(Clone, Copy)]
pub(super) enum Interval {
    Year,
    Quarter,
    Month,
    Day,
    Weekday,
    Week,
    Hour,
    Minute,
    Second,
}
impl Interval {
    pub(super) fn resolve(name: &str) -> Option<Self> {
        [
            ("yyyy", Self::Year),
            ("q", Self::Quarter),
            ("m", Self::Month),
            ("y", Self::Day),
            ("d", Self::Day),
            ("w", Self::Weekday),
            ("ww", Self::Week),
            ("h", Self::Hour),
            ("n", Self::Minute),
            ("s", Self::Second),
        ]
        .into_iter()
        .find_map(|(spelling, interval)| name.eq_ignore_ascii_case(spelling).then_some(interval))
    }
}
