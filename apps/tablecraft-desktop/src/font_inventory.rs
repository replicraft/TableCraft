//! Host-only system family enumeration. No font data enters document services.
use std::sync::OnceLock;
use tablecraft_ui::FontFamilies;

static FAMILIES: OnceLock<FontFamilies> = OnceLock::new();

pub(super) fn families() -> FontFamilies {
    FAMILIES.get_or_init(discover).clone()
}

fn discover() -> FontFamilies {
    let _span = tracing::debug_span!("font_families_discover").entered();
    let mut collection = fontique::Collection::new(fontique::CollectionOptions::default());
    match FontFamilies::from_system(collection.family_names()) {
        Ok(families) => families,
        Err(_) => {
            tracing::warn!("system font family inventory refused; bundled fonts remain available");
            FontFamilies::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_discovery_keeps_bundled_families_and_reuses_the_process_catalog() {
        let first = families();
        let second = families();
        assert_eq!(first.families().as_ptr(), second.families().as_ptr());
        for family in ["Selawik", "Noto Sans JP", "Noto Sans KR", "Noto Sans SC"] {
            assert!(first.families().iter().any(|name| name == family));
        }

        // This independent metadata probe makes the local provider's result
        // observable without requiring a platform-specific installed-font set.
        let mut system = fontique::Collection::new(fontique::CollectionOptions::default());
        let discovered = system.family_names().count();
        println!(
            "Fontique system families: {discovered}; merged catalog: {}",
            first.families().len()
        );
    }
}
