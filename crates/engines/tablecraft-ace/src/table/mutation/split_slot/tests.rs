//! Forged staging cannot replace immutable requests or append authority.
use super::super::{EditError, SlotScalarUpdate, index_append};
use super::{Operation, Staged, finish, preflight, stage};
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints, read_table,
    table::{indexes::IndexDataPatch, mutation::gates},
    test_support::encoded::model,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const ROOT: usize = 13;
const WRITABLE_FLAGS: usize = 10 * PAGE + 115;
const FREE_OFFSET: usize = 2;
const PREFIX_OFFSET: usize = 24;

fn source() -> Vec<u8> {
    let mut source = model::text(1, false, true).bytes;
    source[WRITABLE_FLAGS] = 2;
    source
}

fn prepared<'a>(image: &NativeImage, operation: &Operation<'a>) -> Staged<'a> {
    let (structural, column) =
        preflight(image, operation, &mut model::budget(), &|| false).unwrap();
    stage(
        image,
        operation,
        structural,
        column,
        &mut model::budget(),
        &|| false,
    )
    .unwrap()
}

fn checked(bytes: &[u8], value: &str) {
    let rows = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows()[2][1], Value::Text(value.into()));
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    gates::qualified(
        &check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut model::budget(),
            &|| false,
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn same_weight_correlated_slot_replacement_cannot_change_original_request() {
    let before = source();
    let image = model::capture(&before);
    let requests = [SlotScalarUpdate::new(
        2,
        Value::Text("a".into()),
        Value::Text("b".into()),
    )];
    let operation = Operation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    let healthy = prepared(&image, &operation);
    let candidate = finish::finish(
        model::capture(&before),
        &operation,
        &healthy,
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    checked(candidate.candidate_bytes(), "b");

    // General case weights are equal, but raw stored values and immutable
    // expected/replacement operands remain distinct authority.
    let changed = [SlotScalarUpdate::new(
        2,
        Value::Text("a".into()),
        Value::Text("B".into()),
    )];
    let changed_operation = Operation {
        table: "Items",
        column: "Score",
        requests: &changed,
    };
    let forged = prepared(&image, &changed_operation);
    let correlated = index_append::compose(
        &image,
        &forged.slots.patches,
        &forged.plan,
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    checked(&correlated, "B");
    assert_eq!(
        finish::finish(image, &operation, &forged, &mut model::budget(), &|| false).err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn missing_duplicate_and_other_row_patches_have_no_slot_request_authority() {
    let before = source();
    let requests = [SlotScalarUpdate::new(
        2,
        Value::Text("a".into()),
        Value::Text("b".into()),
    )];
    let operation = Operation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    for mode in 0..3 {
        let image = model::capture(&before);
        let mut forged = prepared(&image, &operation);
        match mode {
            0 => forged.slots.patches.clear(),
            1 => {
                let duplicate = prepared(&image, &operation);
                forged.slots.patches.extend(duplicate.slots.patches);
            }
            _ => {
                let other = [SlotScalarUpdate::new(
                    3,
                    Value::Text("A".into()),
                    Value::Text("b".into()),
                )];
                let other_operation = Operation {
                    table: "Items",
                    column: "Score",
                    requests: &other,
                };
                forged.slots.patches = prepared(&image, &other_operation).slots.patches;
            }
        }
        assert_eq!(
            finish::finish(image, &operation, &forged, &mut model::budget(), &|| false).err(),
            Some(EditError::Preservation)
        );
    }
}

#[test]
fn unrequested_index_keys_residue_and_eof_do_not_gain_patch_authority() {
    let before = source();
    let requests = [SlotScalarUpdate::new(
        2,
        Value::Text("a".into()),
        Value::Text("b".into()),
    )];
    let operation = Operation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    for mode in 0..3 {
        let image = model::capture(&before);
        let mut forged = prepared(&image, &operation);
        match mode {
            0 => {
                let different = [SlotScalarUpdate::new(
                    2,
                    Value::Text("a".into()),
                    Value::Text("z".into()),
                )];
                let other = Operation {
                    table: "Items",
                    column: "Score",
                    requests: &different,
                };
                forged.plan = prepared(&image, &other).plan;
                let actual = index_append::compose(
                    &image,
                    &forged.slots.patches,
                    &forged.plan,
                    &mut model::budget(),
                    &|| false,
                )
                .unwrap();
                let healthy = prepared(&image, &operation);
                let expected = index_append::compose(
                    &image,
                    &healthy.slots.patches,
                    &healthy.plan,
                    &mut model::budget(),
                    &|| false,
                )
                .unwrap();
                // b and z have different complete keys with equal wire width.
                // Prefix/free/layout agreement cannot grant replacement authority.
                for offset in [FREE_OFFSET, PREFIX_OFFSET] {
                    assert_eq!(
                        &actual[ROOT * PAGE + offset..ROOT * PAGE + offset + 2],
                        &expected[ROOT * PAGE + offset..ROOT * PAGE + offset + 2],
                    );
                }
                assert_ne!(
                    &actual[ROOT * PAGE..(ROOT + 1) * PAGE],
                    &expected[ROOT * PAGE..(ROOT + 1) * PAGE],
                );
            }
            1 => {
                let offset = (ROOT + 1) * PAGE - 1;
                forged.plan.patches.push(IndexDataPatch {
                    offset,
                    before: vec![before[offset]],
                    after: vec![0x12],
                });
                let bytes = index_append::compose(
                    &image,
                    &forged.slots.patches,
                    &forged.plan,
                    &mut model::budget(),
                    &|| false,
                )
                .unwrap();
                checked(&bytes, "b");
            }
            _ => {
                forged.plan.length += PAGE;
            }
        }
        assert_eq!(
            finish::finish(image, &operation, &forged, &mut model::budget(), &|| false).err(),
            Some(EditError::Preservation)
        );
    }
}
