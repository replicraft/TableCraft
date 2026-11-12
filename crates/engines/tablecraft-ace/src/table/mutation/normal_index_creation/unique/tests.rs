//! UNIQUE candidates are checked against literal ordinary-source index images.
use crate::{NativeImage, UniqueLongIndexCreate};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::unique_workflow as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024 * 1024, 2_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(31801),
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
fn unique_candidates_include_nullable_rows_direction_and_multileaf_membership() {
    let values = (0..3000)
        .map(|value| Some(value - 1500))
        .collect::<Vec<_>>();
    for descending in [false, true] {
        for population in [
            Vec::new(),
            vec![None, Some(-7), None, Some(0), Some(9)],
            values.to_vec(),
        ] {
            let source = raw::creation(descending, &population);
            let candidate = image(&source.before)
                .prepare_retained_unique_long_index_creation(
                    source.table,
                    &UniqueLongIndexCreate::new(source.index_name, source.column, descending),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(candidate.before_bytes(), &source.before);
            same(candidate.candidate_bytes(), &source.expected);
            assert_eq!(candidate.indexed_rows(), population.len());
            assert_eq!(candidate.logical_index_number(), 0);
            assert_eq!(candidate.physical_index_number(), 0);
            assert_eq!(candidate.descending(), descending);
            assert_eq!(candidate.root_page(), source.root);
            if population.len() == 3000 {
                assert!(source.leaf_pages > 2);
                assert!(candidate.height() >= 1);
            }
        }
    }
}

#[test]
fn duplicate_source_cancel_and_budget_refuse_before_any_candidate_is_published() {
    let duplicate = raw::duplicate_before();
    assert!(
        image(&duplicate)
            .prepare_retained_unique_long_index_creation(
                "UniqueValues",
                &UniqueLongIndexCreate::new("ValueUnique", "CopiedScore", false),
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
    let source = raw::creation(false, &[Some(1), Some(2)]);
    for cancelled in [false, true] {
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            image(&source.before)
                .prepare_retained_unique_long_index_creation(
                    source.table,
                    &UniqueLongIndexCreate::new(source.index_name, source.column, false),
                    &mut work,
                    &|| cancelled,
                )
                .is_err()
        );
    }
}

#[test]
fn unique_create_preserves_complete_owned_twenty_hop_forwarding_source() {
    for descending in [false, true] {
        let source = raw::forwarded(descending);
        let data = (source.target_definition as usize + 1) * 4096;
        assert_eq!(
            u16::from_le_bytes(source.before[data + 12..data + 14].try_into().unwrap()),
            23
        );
        assert_eq!(
            u32::from_le_bytes(
                source.before[source.target_definition as usize * 4096 + 16
                    ..source.target_definition as usize * 4096 + 20]
                    .try_into()
                    .unwrap()
            ),
            3
        );
        let candidate = image(&source.before)
            .prepare_retained_unique_long_index_creation(
                source.table,
                &UniqueLongIndexCreate::new(source.index_name, source.column, descending),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(candidate.before_bytes(), &source.before);
        same(candidate.candidate_bytes(), &source.expected);
        assert_eq!(candidate.indexed_rows(), 3);
        assert_eq!(candidate.root_page(), source.root);
        assert_eq!(
            &candidate.candidate_bytes()[data..data + 4096],
            &source.before[data..data + 4096],
            "every pointer, hidden terminal and historical byte stays unchanged"
        );
    }
}
