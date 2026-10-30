//! Additive expected derivative: abandoned source bodies remain opaque originals.
use super::model::{self, Case, PAGE, Shape};

fn original_roots(shape: Shape) -> &'static [usize] {
    if shape.preserved() {
        &[27, 28, 29]
    } else {
        &[13, 14]
    }
}

pub fn selected_offsets(case: &Case) -> Vec<usize> {
    let first = model::row_span(&case.before, case.data, 0);
    let mut offsets = if case.shape.preserved() {
        vec![first.end - 1]
    } else {
        vec![first.start + 6, case.roots[1] * PAGE + 484]
    };
    offsets.sort_unstable();
    offsets
}

pub fn abandoned_pages(case: &Case) -> Vec<usize> {
    let mut abandoned = Vec::new();
    if case.data != 12 {
        abandoned.push(12);
    }
    for (&old, &new) in original_roots(case.shape).iter().zip(&case.roots) {
        if old != new {
            abandoned.push(old);
        }
    }
    abandoned
}

pub fn case(code: u8, shape: Shape) -> Case {
    // The original constructor remains callable and byte-identical to its first
    // freeze. Its source is moved intact; only its expected image is corrected.
    let original = model::case(code, shape);
    if original.data != 12 {
        let old_row = model::row_span(&original.before, 12, 0);
        let old_selected = if shape.preserved() {
            old_row.end - 1
        } else {
            old_row.start + 6
        };
        assert_ne!(
            original.before[old_selected],
            original.expected[old_selected]
        );
    }
    let selected = selected_offsets(&original);
    let abandoned = abandoned_pages(&original);
    let model::Case {
        before,
        mut expected,
        shape,
        data,
        roots,
        physical_start,
        index_map,
        global_bitmap,
    } = original;

    // Before correction, the original expected must differ only at selected
    // addresses or in the precise abandoned physical bodies. This checks every
    // retained byte without cloning a second pair of large files.
    for (offset, (&old, &new)) in before.iter().zip(&expected).enumerate() {
        if old != new {
            assert!(selected.contains(&offset) || abandoned.contains(&(offset / PAGE)));
        }
    }
    for &number in &abandoned {
        let span = number * PAGE..(number + 1) * PAGE;
        expected[span.clone()].copy_from_slice(&before[span]);
    }

    for &number in &abandoned {
        let span = number * PAGE..(number + 1) * PAGE;
        assert_eq!(&expected[span.clone()], &before[span]);
    }
    for (offset, (&old, &new)) in before.iter().zip(&expected).enumerate() {
        if !selected.contains(&offset) {
            assert_eq!(old, new, "protected original byte {offset}");
        }
    }
    Case {
        before,
        expected,
        shape,
        data,
        roots,
        physical_start,
        index_map,
        global_bitmap,
    }
}
