//! Same-number physical slots cannot borrow another table's certificate identity.
use crate::{
    EditError, IndexCheckStatus, NativeImage, RuleCheckStatus, TableCheck, check_table,
    check_table_constraints,
    table::{
        indexes::{
            MutationKey, fixed_batch_split_patches,
            mutation::{
                IndexKeyUpdate,
                split::fixed_batch::{self, Input},
            },
        },
        mutation::fixed::batch::multitable_tests::model,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024 * 1024, 100_000, 1_000_000_000)
}

#[test]
fn untouched_owned_body_claims_cannot_alias_selected_index_pages_even_for_noops() {
    let case = model::source(model::Settings {
        code: 6,
        descending: true,
        unique: false,
        forwarded: false,
        mode: model::Mode::Full,
    });
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = qualify(&image, case.tables[0].name);
    let plan = fixed_batch_split_patches(
        &image,
        checked.schema(),
        checked.indexes(),
        &[],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        plan.audit_fixed_batches(
            &image,
            &[Input {
                checked: &checked,
                updates: &[]
            }],
            image.as_bytes(),
            &mut budget(),
            &|| false,
        )
        .is_ok()
    );

    // The other table's ordinary-owned map and its first INDEX-owned map
    // have distinct valid storage. Each forged membership claims root 13,
    // while all selected table rows, roots and field declarations stay intact.
    for reference in [21 << 8, 25 << 8] {
        let mut before = case.before.clone();
        let range = model::span(&before, reference);
        assert_eq!(range.len(), 69);
        assert_eq!(&before[range.start..range.start + 5], &[0; 5]);
        let offset = range.start + 5 + 13 / 8;
        let mask = 1 << (13 % 8);
        assert_eq!(before[offset] & mask, 0);
        before[offset] |= mask;
        let conflicting = NativeImage::capture(
            &mut Cursor::new(&before),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let selected = qualify(&conflicting, case.tables[0].name);
        assert!(matches!(
            plan.audit_fixed_batches(
                &conflicting,
                &[Input {
                    checked: &selected,
                    updates: &[]
                }],
                conflicting.as_bytes(),
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Preservation)
        ));
    }
}

fn qualify(image: &NativeImage, table: &str) -> TableCheck {
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 500);
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut image.reader(),
        CatalogScope::new(1),
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 500);
    assert_eq!(constraints.fields().len(), 3);
    assert!(constraints.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    assert!(constraints.relationships().is_empty());
    checked
}

#[test]
fn definition_only_certificate_swap_refuses_after_healthy_literal_proof() {
    let case = model::source(model::Settings {
        code: 6,
        descending: true,
        unique: false,
        forwarded: true,
        mode: model::Mode::Full,
    });
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = NativeImage::capture(
        &mut Cursor::new(&case.expected),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for table in &case.tables {
        qualify(&expected, table.name);
    }
    let checked = case
        .tables
        .iter()
        .map(|table| qualify(&image, table.name))
        .collect::<Vec<TableCheck>>();
    let kind = MutationKey::new(ScalarType::Int32).unwrap();
    let updates = checked
        .iter()
        .zip(&case.tables)
        .map(|(checked, table)| {
            let mut result = Vec::new();
            for (old, new) in table.original.iter().zip(&table.final_rows) {
                for (name, expected, value) in [
                    ("Group", old.left, new.left),
                    ("Score", old.right, new.right),
                ] {
                    if expected == value {
                        continue;
                    }
                    let column = checked
                        .schema()
                        .columns()
                        .iter()
                        .position(|column| column.name() == name)
                        .unwrap();
                    result.push(IndexKeyUpdate {
                        reference: old.reference,
                        column,
                        expected: expected.map(|value| kind.stored(&value.to_le_bytes()).unwrap()),
                        value: value.map(|value| kind.stored(&value.to_le_bytes()).unwrap()),
                    });
                }
            }
            result
        })
        .collect::<Vec<_>>();
    let inputs = checked
        .iter()
        .zip(&updates)
        .map(|(checked, updates)| Input { checked, updates })
        .collect::<Vec<_>>();
    let mut plan = fixed_batch::multiple(&image, &inputs, &mut budget(), &|| false).unwrap();
    assert!(
        plan.audit_fixed_batches(&image, &inputs, &case.expected, &mut budget(), &|| false)
            .is_ok()
    );
    assert_eq!(plan.certificates.len(), 4);
    assert_eq!(
        (
            plan.certificates[0].definition,
            plan.certificates[0].physical
        ),
        (10, 0)
    );
    assert_eq!(
        (
            plan.certificates[2].definition,
            plan.certificates[2].physical
        ),
        (20, 0)
    );
    plan.certificates[0].definition = 20;
    plan.certificates[2].definition = 10;
    assert!(matches!(
        plan.audit_fixed_batches(&image, &inputs, &case.expected, &mut budget(), &|| false),
        Err(EditError::Preservation)
    ));
}
