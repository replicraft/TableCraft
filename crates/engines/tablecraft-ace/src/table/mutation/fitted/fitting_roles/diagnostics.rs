//! Original-source diagnosis only; no candidate builder is invoked.
use super::{budget, capture};
use crate::{
    NativeTableStorage,
    format::{definition, reader::Reader},
    inspect_table_schema,
    table::{
        indexes,
        long_value::maps,
        mutation::{gates, map_authority, mixed_fitted_tests, variable_split::metadata::Metadata},
    },
};
use mixed_fitted_tests::model::{self, Family, Recipe};

#[test]
fn diagnose_old_fitting_original_catalog_ownership() {
    let sources = [
        (
            "legacy",
            model::case(5, Family::Legacy, false, Recipe::Permutation, false),
        ),
        (
            "modern",
            model::case(6, Family::Modern, true, Recipe::Nulls, true),
        ),
        (
            "binary",
            model::case(6, Family::Binary, false, Recipe::Shorter, false),
        ),
        (
            "capacity",
            mixed_fitted_tests::capacity::case(Family::Legacy, false),
        ),
    ];

    for (label, case) in sources {
        let image = capture(&case.before);
        let checked = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
        eprintln!("original {label}: scalar/index/constraint admission passed");

        for origin in image.catalog().origins() {
            if origin.table_storage() != Some(NativeTableStorage::Local) {
                continue;
            }
            let object = image
                .catalog()
                .catalog()
                .objects()
                .iter()
                .find(|object| object.id() == origin.object_id())
                .unwrap();
            let schema = inspect_table_schema(
                &mut image.reader(),
                image.catalog().catalog().scope(),
                object.name().as_str(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let mut source = image.reader();
            let mut work = budget();
            let mut reader = Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget: &mut work,
                cancelled: &|| false,
                reads: 0,
            };
            let native = indexes::definitions_with_tail(&mut reader, &schema).unwrap();
            let raw = definition::read(&mut reader, schema.definition_page()).unwrap();
            let declarations = maps::declarations(&mut reader, &schema).unwrap();
            let end = (native.tail + 12).min(raw.len());
            eprintln!(
                "original {label}: TD={} columns={} physical={} tail={} bytes={:02x?} complete={} declarations={}",
                schema.definition_page(),
                schema.columns().len(),
                schema.layout.physical_indexes,
                native.tail,
                &raw[native.tail..end],
                declarations.complete,
                declarations.entries.len(),
            );
        }

        let census =
            map_authority::capture_catalog(&image, &mut budget(), &|| false, |_, _, _, _| Ok(()));
        eprintln!(
            "original {label}: complete catalog census={:?}",
            census.err()
        );
        let fitted = Metadata::capture_for_fitted_updates(
            &image,
            checked.schema(),
            checked.indexes(),
            &[1, 2],
            &mut budget(),
            &|| false,
        );
        eprintln!("original {label}: fitting protection={:?}", fitted.err());
    }
}
