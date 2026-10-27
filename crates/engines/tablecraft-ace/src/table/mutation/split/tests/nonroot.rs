//! Forged non-root plans cannot enlarge link, separator, map or residual authority.
use super::super::{Operation, Staged, finish, stage};
use crate::{
    Ace16WritePreview, EditError, Int32Update,
    table::indexes::IndexDataPatch,
    test_support::{nonroot_model as model, split_model::budget},
};

fn forged(change: impl FnOnce(&mut Staged)) -> Result<Ace16WritePreview, EditError> {
    let before = model::source(6, false);
    let operation = Operation {
        table: "Items",
        column: "Score",
        requests: &[Int32Update::new(499, 500, i32::MAX)],
    };
    let image = crate::test_support::split_model::capture(&before);
    let mut staged = stage(&image, &operation, &mut budget(), &|| false)?;
    let healthy = finish::finish(
        crate::test_support::split_model::capture(&before),
        &operation,
        &staged,
        &mut budget(),
        &|| false,
    )?;
    model::assert_checked(healthy.candidate_bytes());
    change(&mut staged);
    finish::finish(image, &operation, &staged, &mut budget(), &|| false)
}

fn alter(staged: &mut Staged, offset: usize) {
    let patch = staged
        .plan
        .patches
        .iter_mut()
        .find(|patch| (patch.offset..patch.offset + patch.after.len()).contains(&offset))
        .unwrap();
    patch.after[offset - patch.offset] ^= 1;
}

#[test]
fn nonroot_neighbor_links_full_row_separator_and_stationary_residual_are_certified() {
    for offset in [
        model::FIRST * model::PAGE + 16,
        model::ROOT * model::PAGE + 501,
    ] {
        assert_eq!(
            forged(|staged| alter(staged, offset)).err(),
            Some(EditError::Preservation)
        );
    }
    assert_eq!(
        forged(|staged| {
            let before = model::source(6, false);
            let offset = (model::LAST + 1) * model::PAGE - 1;
            staged.plan.patches.push(IndexDataPatch {
                offset,
                before: vec![before[offset]],
                after: vec![before[offset] ^ 1],
            });
        })
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn nonroot_new_left_residue_and_unused_future_free_bits_are_certified() {
    assert_eq!(
        forged(|staged| {
            staged.plan.added[0].bytes[model::PAGE - 1] = 1;
        })
        .err(),
        Some(EditError::Preservation)
    );
    assert_eq!(
        forged(|staged| {
            let global = staged
                .plan
                .append
                .map_edits
                .iter_mut()
                .find(|edit| edit.offset / model::PAGE == 1)
                .unwrap();
            global.after &= !8;
        })
        .err(),
        Some(EditError::Preservation)
    );
}
