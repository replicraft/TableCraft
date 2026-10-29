//! Same-length original source pair exposing unreachable owned INDEX storage.
use super::{budget, model, qualify};
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, NativeImage, WriteBlocker,
    table::mutation::insert::tests::multiple_indexes::model as frames,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub(in crate::table::mutation::fixed::batch) struct Sources {
    pub good: Vec<u8>,
    pub bad: Vec<u8>,
    pub original: Vec<model::Row>,
}

pub(in crate::table::mutation::fixed::batch) fn sources(code: u8) -> Sources {
    let case = model::source(code, model::Shape::Mixed, false, false, false);
    let mut good = case.before;
    // Both originals already contain the same allocated but unclaimed page18.
    // The only difference is the bad source's INDEX-owned membership bit.
    let orphan = good[13 * model::PAGE..14 * model::PAGE].to_vec();
    good.extend_from_slice(&orphan);
    frames::map_page(&mut good, 1, vec![frames::map(19..512)]);
    let mut bad = good.clone();
    frames::map_page(&mut bad, 15, vec![frames::map([13, 18]), frames::map([14])]);
    Sources {
        good,
        bad,
        original: case.original,
    }
}

#[test]
fn qualify_unreachable_owned_leaf_sources_before_preview() {
    for code in [5, 6] {
        let source = sources(code);
        qualify(&source.good, &source.original, [700, 800]);
        qualify(&source.bad, &source.original, [700, 800]);
        assert_eq!(source.good.len(), 19 * model::PAGE);
        assert_eq!(source.bad.len(), source.good.len());
        let differences = source
            .good
            .iter()
            .zip(&source.bad)
            .enumerate()
            .filter_map(|(offset, (left, right))| (left != right).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(differences.len(), 1);
        let owned = model::span(&source.good, 15 << 8);
        assert_eq!(differences, vec![owned.start + 7]);
        assert_eq!(source.good[differences[0]] ^ source.bad[differences[0]], 4);
    }
}

#[test]
fn unreachable_owned_index_storage_cannot_obtain_mutation_spans() {
    for code in [5, 6] {
        let source = sources(code);
        let updates = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(7))];
        let groups = [FixedColumnUpdates::new("Items", "Group", &updates)];
        let good = NativeImage::capture(
            &mut Cursor::new(&source.good),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let candidate = good
            .preview_ace16_split_batch_updates(&groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.candidate_bytes().len(), source.good.len());
        assert_eq!(candidate.changed_rows(), 1);
        assert_eq!(
            candidate.candidate_bytes()[18 * model::PAGE..],
            source.good[18 * model::PAGE..]
        );

        let bad = NativeImage::capture(
            &mut Cursor::new(&source.bad),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            bad.preview_ace16_split_batch_updates(&groups, &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
}
