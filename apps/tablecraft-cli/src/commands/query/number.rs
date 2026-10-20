use tablecraft_expressions::functions::conversions::text::NumberFormat;
pub(super) fn parse_format(text: &str) -> Result<NumberFormat, String> {
    NumberFormat::resolve(text).ok_or_else(|| "expected invariant, en-us or de-de".into())
}
