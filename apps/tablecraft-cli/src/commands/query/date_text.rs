use tablecraft_expressions::functions::text::cstr::DateTextFormat;
pub(super) fn parse_format(text: &str) -> Result<DateTextFormat, String> {
    DateTextFormat::resolve(text).ok_or_else(|| "expected iso, en-us or de-de".into())
}
