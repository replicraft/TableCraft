//! Complete independent OFF images, preservation and source/budget refusals.
use super::*;
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_creation_primary, canonical_database_options,
    canonical_native17_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(997);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 50_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn originals() -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    for case in [
        canonical_creation::Case::Empty,
        canonical_creation::Case::One,
    ] {
        result.push(canonical_native17_creation::source(
            canonical_creation::source(case).bytes,
        ));
    }
    for case in [
        canonical_creation_primary::Case::Empty,
        canonical_creation_primary::Case::One,
    ] {
        let manual = canonical_creation_primary::source(case).bytes;
        let mut generated = manual.clone();
        // The independent two-Int32 primary source has a 75-byte prefix.
        assert_eq!(generated[7 * 4096 + 75 + 15], 3);
        generated[7 * 4096 + 75 + 15] = 7;
        result.push(manual);
        result.push(generated);
    }
    result
}

#[test]
fn explicit_off_matches_six_complete_independent_images_without_changing_originals() {
    for mut before in originals() {
        before[4 * 4096 + 512..4 * 4096 + 528].copy_from_slice(b"catalog-sentinel");
        before[3 * 4096 + 512..3 * 4096 + 528].copy_from_slice(b"map-gap-sentinel");
        before[7 * 4096 + 3500..7 * 4096 + 3516].copy_from_slice(b"retained-opaque!");
        let expected = canonical_database_options::explicit_off(&before);
        let plan = capture(&before)
            .plan_disable_name_auto_correct(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.property_page() as usize, before.len() / 4096);
        assert_eq!(
            &plan.after_bytes()[7 * 4096..before.len()],
            &before[7 * 4096..]
        );
        assert!(
            capture(plan.after_bytes())
                .plan_disable_name_auto_correct(&mut budget(), &|| false)
                .is_err()
        );
        assert!(
            capture(plan.after_bytes())
                .plan_indexed_table_rename("Items", "Renamed", &mut budget(), &|| false)
                .is_err()
        );
    }
}

#[test]
fn catalog_lval_aliases_payloads_and_noncreator_roles_cannot_gain_option_authority() {
    let before = originals().remove(0);
    for offset in [
        2 * 4096 + 16,
        2 * 4096 + 51,
        3 * 4096 + 8,
        4 * 4096 + 2,
        7 * 4096 + 55,
    ] {
        let mut changed = before.clone();
        changed[offset] ^= 1;
        if let Ok(image) =
            NativeImage::capture(&mut Cursor::new(&changed), SCOPE, &mut budget(), &|| false)
        {
            assert!(
                image
                    .plan_disable_name_auto_correct(&mut budget(), &|| false)
                    .is_err()
            );
        }
    }
    // Alias LvProp owned with its candidate: distinct source map roles matter.
    let mut alias = before.clone();
    let definition_length =
        u32::from_le_bytes(alias[2 * 4096 + 8..2 * 4096 + 12].try_into().unwrap()) as usize;
    let declarations = 2 * 4096 + definition_length - 2 - 7 * 10;
    let owned = declarations + 4 * 10 + 2;
    let candidate = owned + 4;
    let value = alias[owned..owned + 4].to_vec();
    alias[candidate..candidate + 4].copy_from_slice(&value);
    assert!(
        capture(&alias)
            .plan_disable_name_auto_correct(&mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn cancellation_and_exhausted_pools_publish_no_database_option_candidate() {
    let before = originals().remove(0);
    assert!(
        capture(&before)
            .plan_disable_name_auto_correct(&mut budget(), &|| true)
            .is_err()
    );
    for limits in [
        ResourceBudget::new(0, 1_000_000, 50_000_000),
        ResourceBudget::new(512 * 1024 * 1024, 0, 50_000_000),
        ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 0),
    ] {
        let mut limits = limits;
        assert!(
            capture(&before)
                .plan_disable_name_auto_correct(&mut limits, &|| false)
                .is_err()
        );
    }
}
