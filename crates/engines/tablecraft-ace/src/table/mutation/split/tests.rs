//! Causal after-audit forgeries start with real, fully qualified production staging.
mod composite_prefix;
mod fixed;
mod map_growth;
mod nonroot;
mod parent;
mod repeated;

use super::super::physical::Patch;
use super::{Operation, Staged, finish, stage};
use crate::{
    Ace16WritePreview, EditError, Int32Update,
    limits::MAX_IMAGE_BYTES,
    table::indexes::IndexDataPatch,
    test_support::split_model::{self as model, LEFT, PAGE, RIGHT, ROOT, budget, capture},
};

fn forged(mut change: impl FnMut(&mut Staged)) -> Result<Ace16WritePreview, EditError> {
    let bytes = model::source(6, false, true);
    let image = capture(&bytes);
    let requests = [Int32Update::new(0, 1, i32::MAX)];
    let operation = Operation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    let mut staged = stage(&image, &operation, &mut budget(), &|| false)?;
    let valid = finish::finish(capture(&bytes), &operation, &staged, &mut budget(), &|| {
        false
    })?;
    model::assert_checked(valid.candidate_bytes());
    change(&mut staged);
    finish::finish(image, &operation, &staged, &mut budget(), &|| false)
}

#[test]
fn full_separator_row_bounds_and_original_root_residual_are_certified() {
    for offset in [488, 600] {
        assert_eq!(
            forged(|staged| {
                let root = staged
                    .plan
                    .patches
                    .iter_mut()
                    .find(|patch| patch.offset == ROOT * PAGE)
                    .unwrap();
                // Byte488 is the low physical row-reference byte in the left max.
                // Byte600 is old root residue after the new single node entry.
                root.after[offset] ^= 1;
            })
            .err(),
            Some(EditError::Preservation)
        );
    }
}

#[test]
fn eof_stamp_links_and_unused_payload_are_independently_checked() {
    for (page, offset) in [(LEFT, 8), (LEFT, 16), (RIGHT, PAGE - 1)] {
        assert!(
            forged(|staged| {
                let added = staged
                    .plan
                    .added
                    .iter_mut()
                    .find(|added| added.number as usize == page)
                    .unwrap();
                added.bytes[offset] ^= 1;
            })
            .is_err()
        );
    }
}

#[test]
fn unrelated_original_bytes_and_unrequested_future_global_bits_are_protected() {
    assert_eq!(
        forged(|staged| {
            // An unused original page is not index mutation authority, even if
            // a forged patch supplies an exact before byte and no key is changed.
            assert_eq!(model::source(6, false, true)[9 * PAGE + 100], 0);
            staged.plan.patches.push(IndexDataPatch {
                offset: 9 * PAGE + 100,
                before: vec![0],
                after: vec![1],
            });
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
                .find(|edit| edit.offset / PAGE == 1)
                .unwrap();
            // EOF16/17 are real allocations. Page18 is still outside the candidate
            // and must retain its original future-free bit. Full row checks alone
            // cannot certify that unused bit.
            global.after &= !4;
        })
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn forged_original_identity_and_unaligned_or_overcap_lengths_refuse_before_copy() {
    for kind in 0..4 {
        assert_eq!(
            forged(|staged| {
                match kind {
                    0 => staged.plan.append.original_length -= PAGE,
                    1 => staged.plan.append.length = staged.plan.append.original_length - PAGE,
                    2 => staged.plan.append.length += 1,
                    _ => staged.plan.append.length = MAX_IMAGE_BYTES as usize + PAGE,
                }
            })
            .err(),
            Some(EditError::Preservation)
        );
    }
}

#[test]
fn extra_unindexed_fixed_field_patch_has_no_request_authority() {
    assert_eq!(
        forged(|staged| {
            let original = model::source(6, false, true);
            let source_page = 12;
            let row_start = model::word(&original, source_page * PAGE + 14) as usize;
            let id_offset = source_page * PAGE + row_start + 2;
            assert_eq!(&original[id_offset..id_offset + 4], &1_i32.to_le_bytes());
            // This real Id cell is unindexed. The legitimate request only
            // changes Score; row membership and every index remain consistent
            // even if this unrelated value is also changed by a forged stage.
            staged.patches.push(Patch {
                reference: (source_page as u32) << 8,
                offset: id_offset,
                before: 1_i32.to_le_bytes(),
                after: 2_i32.to_le_bytes(),
            });
        })
        .err(),
        Some(EditError::Preservation)
    );
    for kind in 0..3 {
        assert_eq!(
            forged(|staged| {
                match kind {
                    0 => staged.patches.clear(),
                    1 => staged.patches[0].after = (i32::MAX - 1).to_le_bytes(),
                    _ => {
                        let original = model::source(6, false, true);
                        let row_start = model::word(&original, 12 * PAGE + 16) as usize;
                        staged.patches[0] = Patch {
                            reference: (12_u32 << 8) | 1,
                            offset: 12 * PAGE + row_start + 6,
                            before: 2_i32.to_le_bytes(),
                            after: i32::MAX.to_le_bytes(),
                        };
                    }
                }
            })
            .err(),
            Some(EditError::Preservation)
        );
    }
}
