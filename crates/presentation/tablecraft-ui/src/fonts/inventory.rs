//! Inert family names supplied by a host; this module never discovers or loads fonts.
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

const MAX_FAMILIES: usize = 16_384;
const MAX_NAME_BYTES: usize = 4096;
const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;
const BUNDLED_FAMILIES: [&str; 5] = [
    super::SELAWIK,
    super::NOTO_EMOJI,
    super::NOTO_JAPANESE,
    super::NOTO_KOREAN,
    super::NOTO_SIMPLIFIED_CHINESE,
];

/// Host-supplied family names, including the bundled UI fonts.
/// Listing a family does not load it into the renderer or apply document formatting.
#[derive(Clone, Debug)]
pub struct FontFamilies {
    families: Arc<[String]>,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum FontInventoryError {
    #[error("font family name is empty or contains a control character")]
    InvalidName,
    #[error("font family inventory exceeds its presentation limit")]
    Limit,
}

impl FontFamilies {
    /// Merge system names with bundled names and sort/deduplicate by Unicode
    /// lowercase. This presentation ordering is independent of database collation.
    pub fn from_system<I, S>(names: I) -> Result<Self, FontInventoryError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut families = BTreeMap::new();
        let mut bytes = 0_usize;
        for name in BUNDLED_FAMILIES {
            insert(&mut families, &mut bytes, name)?;
        }
        for (index, name) in names.into_iter().enumerate() {
            if index >= MAX_FAMILIES - BUNDLED_FAMILIES.len() {
                return Err(FontInventoryError::Limit);
            }
            insert(&mut families, &mut bytes, name.as_ref())?;
        }

        Ok(Self {
            families: families.into_values().collect::<Vec<_>>().into(),
        })
    }

    pub fn families(&self) -> &[String] {
        &self.families
    }
}

fn insert(
    families: &mut BTreeMap<String, String>,
    bytes: &mut usize,
    name: &str,
) -> Result<(), FontInventoryError> {
    if name.len() > MAX_NAME_BYTES {
        return Err(FontInventoryError::Limit);
    }
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err(FontInventoryError::InvalidName);
    }
    *bytes = bytes
        .checked_add(name.len())
        .ok_or(FontInventoryError::Limit)?;
    if *bytes > MAX_TOTAL_BYTES {
        return Err(FontInventoryError::Limit);
    }
    families
        .entry(name.to_lowercase())
        .or_insert_with(|| name.to_owned());
    Ok(())
}

impl Default for FontFamilies {
    fn default() -> Self {
        Self::from_system(std::iter::empty::<&str>()).expect("fixed bundled font names are valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::{Preview, Scene};

    #[test]
    fn host_names_are_sorted_deduplicated_and_keep_bundled_canonical_names() {
        let fonts =
            FontFamilies::from_system(["zFont", "Arial", "arial", "SELAWIK", "ÅFont"]).unwrap();
        let arial_count = fonts
            .families()
            .iter()
            .filter(|name| name.eq_ignore_ascii_case("Arial"))
            .count();
        assert_eq!(arial_count, 1);
        assert!(fonts.families().iter().any(|name| name == "Selawik"));
        assert!(!fonts.families().iter().any(|name| name == "SELAWIK"));
        let ordered = fonts
            .families()
            .windows(2)
            .all(|pair| pair[0].to_lowercase() < pair[1].to_lowercase());
        assert!(ordered);
        for name in BUNDLED_FAMILIES {
            assert!(fonts.families().iter().any(|family| family == name));
        }
    }

    #[test]
    fn malformed_or_unbounded_inventory_is_refused_without_a_partial_list() {
        let malformed = FontFamilies::from_system(["Arial", "\n"]);
        assert_eq!(malformed.unwrap_err(), FontInventoryError::InvalidName);

        let oversized = FontFamilies::from_system(["x".repeat(4097)]);
        assert_eq!(oversized.unwrap_err(), FontInventoryError::Limit);

        let excessive = FontFamilies::from_system(std::iter::repeat_n("Arial", 16_384));
        assert_eq!(excessive.unwrap_err(), FontInventoryError::Limit);
    }

    #[test]
    fn preview_scene_changes_preserve_host_inventory_without_discovering_fonts() {
        let mut preview = Preview::new(Scene::Datasheet);
        let fonts = FontFamilies::from_system(["Independent host font"]).unwrap();
        let expected = fonts.families().to_vec();
        preview.state_mut().font_families = fonts;

        for scene in [Scene::Design, Scene::StartupOpen, Scene::Datasheet] {
            preview.set_scene(scene);
            assert_eq!(preview.state().font_families.families(), expected);
        }
    }
}
