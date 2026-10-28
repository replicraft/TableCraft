//! A matching population count cannot substitute for exact key/ROW membership.
use super::*;
use crate::{
    NativeImage,
    table::mutation::{
        gates,
        insert::{primary_column, tests::multiple_indexes::model},
    },
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

#[test]
fn original_eligibility_is_bound_to_complete_primary_rows_not_just_count() {
    let case = model::ignored_null_source(6);
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = gates::before_insert(&image, "Items", &mut budget(), &|| false).unwrap();
    let id = primary_column::resolve(&checked, &mut budget(), &|| false).unwrap();
    let mut allowance = budget();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    let selections = super::super::selection::capture(&checked, id, &mut reader).unwrap();
    let sources = selections
        .iter()
        .map(|selection| {
            super::super::tree::inventory(
                &image,
                selection.index,
                checked.schema().definition_page(),
                &mut reader,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let references = primary_references(&checked, &selections, &sources, &mut reader)
        .unwrap()
        .unwrap();
    assert_eq!(references, [(12 << 8), (12 << 8) | 1, (12 << 8) | 2]);
    let position = selections
        .iter()
        .position(|selection| selection.profile.ignore_nulls)
        .unwrap();
    let selection = &selections[position];
    let entries = &sources[position].0;
    assert_eq!(entries.len(), 2);
    original(&checked, selection, &references, entries, &mut reader).unwrap();

    // Keep two sorted canonical non-NULL entries, but bind key10 to the actual
    // third root whose stored Group is NULL and must be absent from this index.
    let mut wrong = entries
        .iter()
        .map(|entry| entry.copy_paid(&mut reader).unwrap())
        .collect::<Vec<_>>();
    let mut wire = vec![127, 128, 0, 0, 10];
    wire.extend(((12_u32 << 8) | 2).to_be_bytes());
    wrong[1] = Wire::read(&wire).unwrap();
    wrong.sort();
    assert_eq!(wrong.len(), entries.len());
    assert_eq!(
        original(&checked, selection, &references, &wrong, &mut reader),
        Err(EditError::Preservation)
    );
}

#[test]
fn reversed_ignored_components_decode_declaration_order_without_changing_key_order() {
    use crate::table::mutation::insert::tests::guid_tuple_indexes::model as tuples;
    use tablecraft_core::values::{Guid, Value};

    let case = tuples::source(
        6,
        tuples::Settings {
            ignored: true,
            ..tuples::Settings::ordinary(tuples::Shape::ScoreThenGuid)
        },
        false,
    );
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = gates::before_insert(&image, "Items", &mut budget(), &|| false).unwrap();
    let id = primary_column::resolve(&checked, &mut budget(), &|| false).unwrap();
    let mut allowance = budget();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    let selections = super::super::selection::capture(&checked, id, &mut reader).unwrap();
    let mut sources = Vec::new();
    for selection in &selections {
        sources.push(
            super::super::tree::inventory(
                &image,
                selection.index,
                checked.schema().definition_page(),
                &mut reader,
            )
            .unwrap(),
        );
    }
    let references = primary_references(&checked, &selections, &sources, &mut reader)
        .unwrap()
        .unwrap();
    let selection = &selections[1];
    assert_eq!(selection.profile.columns(&mut reader).unwrap(), [2, 1]);
    let entries = &sources[1].0;
    assert_eq!(entries.len(), 2);
    original(&checked, selection, &references, entries, &mut reader).unwrap();
    assert_eq!(selection.profile.columns(&mut reader).unwrap(), [2, 1]);

    // A canonical two-entry tree with the wrong stored Score still fails exact
    // membership, even after the declaration/key-order bridge is established.
    let mut wrong = entries
        .iter()
        .map(|entry| entry.copy_paid(&mut reader).unwrap())
        .collect::<Vec<_>>();
    wrong[0] = selection
        .profile
        .request(
            &[
                Value::Int32(1),
                Value::Guid(Guid::from_bytes([
                    16, 1, 2, 3, 4, 5, 6, 7, 0x80, 9, 10, 11, 12, 13, 14, 0xff,
                ])),
                Value::Int32(99),
            ],
            references[0],
            &mut reader,
        )
        .unwrap();
    wrong.sort();
    assert_eq!(wrong.len(), entries.len());
    assert_eq!(
        original(&checked, selection, &references, &wrong, &mut reader),
        Err(EditError::Preservation)
    );
}
