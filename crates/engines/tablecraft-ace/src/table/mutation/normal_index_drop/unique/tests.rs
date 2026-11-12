//! Unique DROP preserves row history and retains a surviving alias's whole tree.
use crate::NativeImage;
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{
    self as normal, Population, unique_workflow as raw,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024 * 1024, 2_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(31802),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first complete-image difference"
    );
}

#[test]
fn unique_drop_retires_exclusive_root_map_and_preserves_all_rows_or_surviving_alias() {
    for descending in [false, true] {
        let history = raw::source(descending);
        let source = &history.updated.creation;
        assert!(
            image(&history.omitted)
                .select_native_retained_unique_long_index_drop(
                    source.table,
                    source.index_name,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
        );
        let candidate = image(&history.omitted)
            .prepare_retained_unique_long_index_drop(
                source.table,
                source.index_name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(candidate.before_bytes(), &history.omitted);
        same(candidate.candidate_bytes(), &history.dropped);
        assert_eq!(candidate.retired_pages(), source.index_pages.len() + 1);
        assert!(
            image(&history.omitted)
                .prepare_retained_index_drop(
                    source.table,
                    source.index_name,
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "ordinary DROP source remains closed"
        );

        let alias = raw::alias(descending);
        let source = &alias.creation;
        let candidate = image(&alias.before)
            .prepare_retained_unique_long_index_drop(
                source.table,
                source.index_name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(candidate.candidate_bytes(), &alias.expected);
        assert_eq!(candidate.retired_pages(), 0);
        for page in source.index_pages.iter().copied().chain([source.map_host]) {
            let start = page as usize * 4096;
            assert_eq!(
                &candidate.candidate_bytes()[start..start + 4096],
                &alias.before[start..start + 4096]
            );
        }
    }
}

#[test]
fn ordinary_source_and_cancel_budget_cannot_enter_unique_drop_language() {
    let ordinary = normal::source(Population::NullableDuplicate, false);
    assert!(
        !image(&ordinary.expected)
            .select_native_retained_unique_long_index_drop(
                ordinary.table,
                ordinary.index_name,
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    assert!(
        image(&ordinary.expected)
            .prepare_retained_unique_long_index_drop(
                ordinary.table,
                ordinary.index_name,
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    let history = raw::source(false);
    let source = &history.updated.creation;
    for cancelled in [false, true] {
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            image(&history.omitted)
                .prepare_retained_unique_long_index_drop(
                    source.table,
                    source.index_name,
                    &mut work,
                    &|| cancelled
                )
                .is_err()
        );
    }
}
