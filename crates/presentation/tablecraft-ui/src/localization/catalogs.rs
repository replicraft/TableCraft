use super::Locale;

macro_rules! section {
    ($locale:literal, $section:literal) => {
        (
            $section,
            include_str!(concat!(
                "../../../../../locales/",
                $locale,
                "/",
                $section,
                ".ftl"
            )),
        )
    };
}

macro_rules! sections {
    ($locale:literal) => {
        [
            section!($locale, "shell"),
            section!($locale, "ribbon"),
            section!($locale, "backstage"),
            section!($locale, "creation"),
            section!($locale, "table-draft"),
            section!($locale, "navigation"),
            section!($locale, "objects"),
            section!($locale, "datasheet"),
            section!($locale, "diagnostics"),
            section!($locale, "query"),
            section!($locale, "query-actions"),
            section!($locale, "query-design"),
            section!($locale, "saved-query"),
            section!($locale, "design"),
            section!($locale, "editing"),
            section!($locale, "recovery"),
            section!($locale, "insertion"),
            section!($locale, "deletion"),
            section!($locale, "zoom"),
            section!($locale, "find"),
            section!($locale, "filtering"),
            section!($locale, "totals"),
            section!($locale, "collections"),
            section!($locale, "settings"),
            section!($locale, "database-options"),
            section!($locale, "field-rename"),
            section!($locale, "field-drop"),
            section!($locale, "table-drop"),
        ]
    };
}

pub(super) fn sources(
    locale: Locale,
) -> impl ExactSizeIterator<Item = (&'static str, &'static str)> {
    let sources = match locale {
        Locale::EnUs => sections!("en-US"),
        Locale::Es => sections!("es"),
        Locale::De => sections!("de"),
        Locale::Ru => sections!("ru"),
        Locale::Ja => sections!("ja"),
        Locale::Ko => sections!("ko"),
        Locale::ZhCn => sections!("zh-CN"),
        Locale::PtBr => sections!("pt-BR"),
    };

    sources.into_iter()
}
