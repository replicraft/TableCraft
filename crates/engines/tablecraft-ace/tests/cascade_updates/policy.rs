//! Public batch extensions must not leak through the automatic cascade wrapper.
use super::{budget, capture, foreign_fixture, support, update};
use tablecraft_ace::{
    EditError, FixedColumnUpdates, IndexCheckStatus, RelationshipCheckStatus, RuleCheckStatus,
    WriteBlocker, check_table, check_table_constraints, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

fn span(bytes: &[u8], page: usize, slot: usize) -> std::ops::Range<usize> {
    let base = page * PAGE;
    let start = usize::from(u16::from_le_bytes(
        bytes[base + 14 + slot * 2..base + 16 + slot * 2]
            .try_into()
            .unwrap(),
    )) & 0x1fff;
    let end = if slot == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[base + 12 + slot * 2..base + 14 + slot * 2]
                .try_into()
                .unwrap(),
        )) & 0x1fff
    };
    base + start..base + end
}

fn forwarded(code: u8, descending: bool) -> (Vec<u8>, Vec<u8>) {
    let mut before = foreign_fixture::database(code, descending, true);
    let root = span(&before, 12, 1);
    let body = before[root.clone()].to_vec();
    assert_eq!(body.len(), 11);
    before.resize(18 * PAGE, 0);
    let mut hidden = support::data_page(10, &[(0x8000, body)]);
    support::put16(&mut hidden, 2, 4069);
    before[17 * PAGE..18 * PAGE].copy_from_slice(&hidden);
    before[root.clone()].fill(0);
    before[root.start..root.start + 4].copy_from_slice(&(17_u32 << 8).to_le_bytes());
    let at = 12 * PAGE + 16;
    let word = u16::from_le_bytes(before[at..at + 2].try_into().unwrap());
    before[at..at + 2].copy_from_slice(&(word | 0x4000).to_le_bytes());
    let mut owned = vec![0; 69];
    owned[6] = 0x10;
    owned[7] = 2;
    let mut container = support::data_page(1, &[(0, owned)]);
    support::put16(&mut container, 2, 4011);
    before[11 * PAGE..12 * PAGE].copy_from_slice(&container);

    let mut expected = before.clone();
    let parent = span(&expected, 12, 0);
    expected[parent.start + 2..parent.start + 6].copy_from_slice(&9_i32.to_le_bytes());
    let child = span(&expected, 17, 0);
    expected[child.start + 6..child.start + 10].copy_from_slice(&9_i32.to_le_bytes());
    let child = span(&expected, 12, 2);
    expected[child.start + 6..child.start + 10].copy_from_slice(&9_i32.to_le_bytes());
    expected[13 * PAGE..14 * PAGE]
        .copy_from_slice(&foreign_fixture::index(&[Some(9), Some(2), Some(3)], false));
    expected[16 * PAGE..17 * PAGE].copy_from_slice(&foreign_fixture::index(
        &[None, Some(9), Some(9)],
        descending,
    ));
    (before, expected)
}

fn ignored(code: u8) -> Vec<u8> {
    let mut before = foreign_fixture::database(code, false, true);
    before[10 * PAGE + 253] = 0x82;
    let mut root = before[16 * PAGE..17 * PAGE].to_vec();
    // Omit the original NULL key while retaining both logical child references.
    root.copy_within(485..503, 480);
    root[27..480].fill(0);
    root[27 + 9 / 8] |= 1 << (9 % 8);
    root[27 + 18 / 8] |= 1 << (18 % 8);
    support::put16(&mut root, 2, 3598);
    before[16 * PAGE..17 * PAGE].copy_from_slice(&root);
    before
}

fn qualify(bytes: &[u8], after: bool) {
    let image = capture(bytes);
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let report = check_table_constraints(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.relationships().len(), 1);
    assert_eq!(
        report.relationships()[0].status(),
        RelationshipCheckStatus::Checked
    );
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    let table = read_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let value = if after { 9 } else { 1 };
    assert_eq!(
        table.rows(),
        &[
            vec![Value::Int32(value), Value::Null],
            vec![Value::Int32(2), Value::Int32(value)],
            vec![Value::Int32(3), Value::Int32(value)],
        ]
    );
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(offset, (left, right))| (offset, *left, *right))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "{} differences; first {:?}",
        differences.len(),
        &differences[..differences.len().min(8)]
    );
}

#[test]
fn qualify_cascade_policy_sources_and_explicit_batch_images_before_preview() {
    for code in [5, 6] {
        for descending in [false, true] {
            let (before, expected) = forwarded(code, descending);
            qualify(&before, false);
            qualify(&expected, true);
            let root = span(&before, 12, 1);
            assert_eq!(before[root.clone()], expected[root]);
            assert_eq!(before[11 * PAGE..12 * PAGE], expected[11 * PAGE..12 * PAGE]);
            assert_eq!(
                before[17 * PAGE..17 * PAGE + 16],
                expected[17 * PAGE..17 * PAGE + 16]
            );
        }
        qualify(&ignored(code), false);
    }
}

#[test]
fn public_explicit_batch_can_patch_forwarded_children_but_automatic_cascade_stays_strict() {
    for code in [5, 6] {
        for descending in [false, true] {
            let (before, expected) = forwarded(code, descending);
            let parent = [update(0, 1, 9)];
            let children = [update(1, 1, 9), update(2, 1, 9)];
            let result = capture(&before)
                .preview_ace16_batch_updates(
                    &[
                        FixedColumnUpdates::new("Items", "Id", &parent),
                        FixedColumnUpdates::new("Items", "Score", &children),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_image(result.candidate_bytes(), &expected);
            assert_eq!(
                capture(&before)
                    .preview_ace16_cascading_updates(
                        &[FixedColumnUpdates::new("Items", "Id", &parent)],
                        &mut budget(),
                        &|| false,
                    )
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::RowLayout))
            );
        }
    }
}

#[test]
fn cascade_empty_no_fact_and_active_routes_do_not_inherit_ignored_index_admission() {
    for code in [5, 6] {
        let before = ignored(code);
        let no_tables = capture(&before)
            .preview_ace16_cascading_updates(&[], &mut budget(), &|| false)
            .unwrap();
        assert_image(no_tables.candidate_bytes(), &before);
        let empty = [];
        let no_facts = [FixedColumnUpdates::new("Items", "Score", &empty)];
        let preview = capture(&before)
            .preview_ace16_batch_updates(&no_facts, &mut budget(), &|| false)
            .unwrap();
        assert_image(preview.candidate_bytes(), &before);
        assert_eq!(
            capture(&before)
                .preview_ace16_cascading_updates(&no_facts, &mut budget(), &|| false,)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
        let parent = [update(0, 1, 9)];
        assert_eq!(
            capture(&before)
                .preview_ace16_cascading_updates(
                    &[FixedColumnUpdates::new("Items", "Id", &parent)],
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
    }
}
