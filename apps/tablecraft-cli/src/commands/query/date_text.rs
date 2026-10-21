use tablecraft_expressions::functions::text::cstr::DateTextFormat;
pub(super) fn parse_format(text: &str) -> Result<DateTextFormat, String> {
    DateTextFormat::resolve(text).ok_or_else(|| "expected iso, en-us or de-de".into())
}

pub(super) fn parse_reference_year(
    text: &str,
) -> Result<tablecraft_expressions::functions::dates::parsing::ReferenceYear, String> {
    text.parse::<u16>()
        .ok()
        .and_then(|year| {
            tablecraft_expressions::functions::dates::parsing::ReferenceYear::new(year).ok()
        })
        .ok_or_else(|| "expected a Gregorian reference year from 100 through 9999".into())
}
