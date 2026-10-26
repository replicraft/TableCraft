use tablecraft_query::BindingDepth;

pub(super) fn parse(text: &str) -> Result<BindingDepth, String> {
    text.parse::<usize>()
        .ok()
        .and_then(|depth| BindingDepth::new(depth).ok())
        .ok_or_else(|| {
            format!(
                "expected binding depth from 1 through {}",
                BindingDepth::MAX
            )
        })
}
