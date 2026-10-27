//! Final certification rebinds map authority to the immutable original catalog.
use super::super::{Operation, admission, finish, stage, tests::model};
use crate::{EditError, WriteBlocker, test_support::support};
use tablecraft_core::values::Value;

fn original(shared: bool) -> Vec<u8> {
    let mut bytes = model::source(6, 1);
    model::set_maps(&mut bytes, &[], &[], &[model::OLE]);
    let row = model::record(
        1,
        Some(&model::inline(&[65, 0])),
        Some(&model::single(1, model::OLE, 0)),
    );
    model::install(&mut bytes, model::ROOT, &model::page(10, &[(0, row)]));
    model::other_table_alias(&mut bytes, true, false);
    if shared {
        support::put32(&mut bytes, 15 * 4096 + 131, (11 << 8) | 2);
        support::put32(&mut bytes, 15 * 4096 + 135, (11 << 8) | 3);
    }
    bytes
}

#[test]
fn finalizer_rechecks_empty_map_identity_against_the_original_catalog() {
    let healthy = original(false);
    let requests = [model::memo_update(Value::Text("Ā".repeat(33)))];
    let image = model::capture(&healthy);
    let prepared =
        admission::prepare(&image, "Items", "Memo", &mut model::budget(), &|| false).unwrap();
    let staged = stage::build(&image, &prepared, &requests, &mut model::budget(), &|| {
        false
    })
    .unwrap();
    let operation = Operation {
        table: "Items",
        column: "Memo",
        updates: &requests,
    };
    let candidate = finish::finish(
        model::capture(&healthy),
        &operation,
        &staged,
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        model::read(candidate.candidate_bytes())[0][1],
        Value::Text("Ā".repeat(33))
    );

    // The selected row/map before-bytes still match this staged plan, while the
    // original unrelated declaration now also claims its empty mutable maps.
    let shared = original(true);
    assert_eq!(model::check(&shared).columns()[0].owned_pages(), 0);
    assert_eq!(
        finish::finish(
            model::capture(&shared),
            &operation,
            &staged,
            &mut model::budget(),
            &|| false
        )
        .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
}
