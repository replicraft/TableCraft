//! Complete raw AFTERs are authored separately from native parsers and writers.
use super::*;
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

const SCOPE: CatalogScope = CatalogScope::new(986);
const CASES: [SourceCase; 4] = [
    SourceCase::NumberFirst,
    SourceCase::NumberHistory,
    SourceCase::InlineNumberThenText,
    SourceCase::TextNumberText,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first differing offsets: {:?}",
        actual
            .iter()
            .zip(expected)
            .enumerate()
            .filter_map(|(offset, (left, right))| (left != right).then_some(offset))
            .take(16)
            .collect::<Vec<_>>()
    );
}

#[test]
fn grow_shrink_and_maximum_ascii_names_match_independent_complete_images() {
    for case in CASES {
        for name in ["TaxAmount".to_owned(), "X".to_owned(), "N".repeat(64)] {
            let [before, after] = fitting::pair(case, &name);
            let original = fitting::old_name(case);
            let source = image(&before);
            let facts = source
                .inspect_fitted_field_rename_source("Table1", original, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(facts.maximum_name_utf16_units(), 64);
            assert_eq!(facts.identity().name_utf16_units(), original.len());
            let candidate = source
                .prepare_fitted_field_rename("Table1", original, &name, &mut budget(), &|| false)
                .unwrap();
            equal_image(candidate.before_bytes(), &before);
            equal_image(candidate.after_bytes(), &after);
            assert_eq!(candidate.table(), "Table1");
            assert_eq!(candidate.column(), original);
            assert_eq!(candidate.new_name(), name);
            assert_eq!(candidate.identity(), facts.identity());
            assert!(candidate.changed());
        }
    }
}

#[test]
fn actual_owned_page_capacity_accepts_twenty_eight_units_and_refuses_twenty_nine() {
    let name = "R".repeat(28);
    let [before, after] = fitting::near_capacity_pair(&name);
    let original = image(&before);
    let facts = original
        .inspect_fitted_field_rename_source("Table1", "Score", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(facts.identity().name_utf16_units(), 5);
    assert_eq!(facts.maximum_name_utf16_units(), 28);
    let candidate = original
        .prepare_fitted_field_rename("Table1", "Score", &name, &mut budget(), &|| false)
        .unwrap();
    equal_image(candidate.before_bytes(), &before);
    equal_image(candidate.after_bytes(), &after);

    assert!(matches!(
        image(&before).prepare_fitted_field_rename(
            "Table1",
            "Score",
            &"R".repeat(29),
            &mut budget(),
            &|| false,
        ),
        Err(Error::FittedCapacity)
    ));
}

#[test]
fn fitted_and_existing_same_width_languages_remain_separate() {
    let before = fitting::source(SourceCase::NumberFirst);
    assert!(matches!(
        image(&before).prepare_same_width_field_rename(
            "Table1",
            "Score",
            "TaxAmount",
            &mut budget(),
            &|| false,
        ),
        Err(Error::NameShape)
    ));
    for same_width in ["Score", "Total"] {
        assert!(matches!(
            image(&before).prepare_fitted_field_rename(
                "Table1",
                "Score",
                same_width,
                &mut budget(),
                &|| false,
            ),
            Err(Error::FittedNameShape)
        ));
    }
    let unchanged = image(&before)
        .prepare_same_width_field_rename("Table1", "Score", "Score", &mut budget(), &|| false)
        .unwrap();
    assert!(!unchanged.changed());
    equal_image(unchanged.after_bytes(), &before);

    for name in ["", ".", &"a".repeat(65)] {
        assert!(matches!(
            image(&before).prepare_fitted_field_rename(
                "Table1",
                "Score",
                name,
                &mut budget(),
                &|| false,
            ),
            Err(Error::InvalidName)
        ));
    }
    assert!(matches!(
        image(&before).prepare_fitted_field_rename(
            "Table1",
            "Score",
            "Résumé",
            &mut budget(),
            &|| false,
        ),
        Err(Error::FittedNameShape)
    ));
    assert!(matches!(
        image(&before)
            .prepare_fitted_field_rename("Table1", "Score", "ID", &mut budget(), &|| false,),
        Err(Error::DuplicateName)
    ));
}

#[test]
fn relocation_verifier_protects_payloads_descriptors_and_newly_free_stale_bytes() {
    for name in ["TaxAmount", "X"] {
        let before = fitting::source(SourceCase::TextNumberText);
        let source =
            Qualified::capture(image(&before), "Table1", "Label", &mut budget(), &|| false)
                .unwrap();
        let layout = Layout::qualified(&source, &mut budget(), &|| false).unwrap();
        let delta = (name.len() as isize - "Label".len() as isize) * 2;
        let after =
            build::candidate(&source, &layout, name, delta, &mut budget(), &|| false).unwrap();
        verify::candidate(
            &source,
            &layout,
            &after,
            name,
            delta,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut offsets = vec![
            source.observed.definition_entry.start,
            source.observed.property_descriptor.start,
            source.observed.property_descriptor.start + 4,
            layout.definition_base
                + build::shifted(source.observed.schema.physical_layout().index_start, delta)
                    .unwrap(),
            layout.property_base + build::shifted(layout.database_start, -delta).unwrap(),
            layout.property_base + layout.property_end,
        ];
        if delta < 0 {
            offsets.push(
                layout.definition_base + build::shifted(layout.definition_length, delta).unwrap(),
            );
            offsets.push(layout.property_base + layout.database_start);
        }
        for offset in offsets {
            let mut forged = after.clone();
            forged[offset] ^= 1;
            assert!(matches!(
                verify::candidate(
                    &source,
                    &layout,
                    &forged,
                    name,
                    delta,
                    &mut budget(),
                    &|| false
                ),
                Err(Error::Preservation)
            ));
        }
    }
}

#[test]
fn cancellation_and_corrupt_settled_sources_do_not_issue_fitted_facts_or_candidates() {
    let before = fitting::source(SourceCase::NumberFirst);
    assert!(matches!(
        image(&before).inspect_fitted_field_rename_source(
            "Table1",
            "Score",
            &mut budget(),
            &|| true
        ),
        Err(Error::SourceProof(
            crate::FieldHistoryDatabaseOptionsError::Inspect(crate::InspectError::Cancelled)
        ))
    ));
    let mut corrupt = before;
    corrupt[7 * 4096 + 4095] = 1;
    let source = image(&corrupt);
    assert!(
        source
            .inspect_fitted_field_rename_source("Table1", "Score", &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        source
            .prepare_fitted_field_rename("Table1", "Score", "TaxAmount", &mut budget(), &|| false)
            .is_err()
    );
}
