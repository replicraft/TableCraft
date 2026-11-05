use super::Locale;

macro_rules! section {
    ($locale:literal, $section:literal) => {
        (
            $section,
            include_str!(concat!(
                "../../../../../assets/ui/locales/",
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
            section!($locale, "datasheet"),
            section!($locale, "query"),
            section!($locale, "saved-query"),
            section!($locale, "design"),
            section!($locale, "editing"),
            section!($locale, "collections"),
            section!($locale, "settings"),
        ]
    };
}

pub(super) fn sources(locale: Locale) -> [(&'static str, &'static str); 10] {
    match locale {
        Locale::EnUs => sections!("en-US"),
        Locale::Es => sections!("es"),
        Locale::De => sections!("de"),
        Locale::Ru => sections!("ru"),
        Locale::Ja => sections!("ja"),
        Locale::Ko => sections!("ko"),
        Locale::ZhCn => sections!("zh-CN"),
        Locale::PtBr => sections!("pt-BR"),
    }
}
