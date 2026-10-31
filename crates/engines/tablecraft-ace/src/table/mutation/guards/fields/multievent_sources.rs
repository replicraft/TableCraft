//! Inert complete-registry images before any multi-event preparation consumer.
mod api;
mod causal;
mod model;

use super::physical_sources;
use crate::{
    IndexCheckStatus, InspectError, NativeImage, TableExtraCoverage, check_table,
    check_table_constraints, read_table, read_table_index,
    table::mutation::{fixed::extended, gates},
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 500_000_000)
}

fn qualify(case: &model::Case, after: bool) {
    let images = &case.images;
    let (bytes, rows) = if after {
        (&images.expected, &images.final_rows)
    } else {
        (&images.before, &images.original)
    };
    let scope = CatalogScope::new(7);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 500);
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    gates::qualified(
        &check_table_constraints(&mut image.reader(), scope, "Items", &mut budget(), &|| {
            false
        })
        .unwrap(),
    )
    .unwrap();
    let table = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(
        table.rows(),
        rows.iter()
            .map(physical_sources::values)
            .collect::<Vec<_>>()
    );

    for (physical, names) in [["LeftOrder", "LeftAlias"], ["PairOrder", "PairAlias"]]
        .into_iter()
        .enumerate()
    {
        let start = (13 + physical) * 4096;
        assert_eq!(
            physical_sources::raw_wires(&bytes[start..start + 4096]),
            physical_sources::model::entries(rows, physical, false),
        );
        for name in names {
            let read = read_table_index(
                &mut image.reader(),
                scope,
                "Items",
                name,
                &mut budget(),
                &|| false,
            );
            if physical == 0 {
                assert_eq!(read.unwrap().rows().len(), 500);
            } else {
                assert_eq!(read.err(), Some(InspectError::Unsupported));
            }
        }
    }

    // This existing helper only inventories original records, catalog body roles
    // and allocation. It stages no row, reserves no page and executes no macro.
    extended::protect_source(
        &image,
        &checked,
        ["ID", "Group", "Score"].into_iter(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let extra = image
        .inspect_table_extra(scope.object_id(10), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
    assert_eq!(extra.raw(), Some(images.extra.as_slice()));
    let properties = extra.groups()[0].properties().unwrap();
    assert_eq!(properties.len(), case.properties.len());
    for ((property, (name, xml)), events) in
        properties.iter().zip(&case.properties).zip(&case.events)
    {
        assert_eq!(property.name(), name);
        assert_eq!(property.text(), Some(xml.as_str()));
        let envelopes = property
            .inspect_macro_xml(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            envelopes
                .definitions()
                .iter()
                .map(|definition| definition.event().unwrap())
                .collect::<Vec<_>>(),
            *events,
        );
    }
    assert!(case.selection.iter().all(|index| *index < properties.len()));
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_complete_multievent_sources_and_expected_before_preparation() {
    let mut pairs = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            qualify(&case, false);
            qualify(&case, true);
            assert_eq!(case.images.before.len(), case.images.expected.len());
            if !case.changed {
                assert_eq!(case.images.before, case.images.expected);
            }
            pairs += 1;
        }
    }
    assert_eq!(pairs, 24);
}
