//! Complete independently authored expectations, read without a batch writer.
use super::super::{joint_fixed_sources::model as fixed_model, source as preserved_source};
use super::{model as sources, required as required_sources};
#[path = "expectations/model.rs"]
pub(crate) mod model;

use crate::{
    BeforeEventGuardSelector, NativeImage, PayloadPresence, TableExtraCoverage,
    inspect_table_indexes, inspect_table_properties, inspect_table_schema, read_attachment_column,
    read_stored_int32_calculation, read_table_projection,
    table::mutation::{
        guards::fields::batch::SelectedPrograms, variable_split::metadata::Metadata,
    },
};
use std::{collections::BTreeSet, io::Cursor};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn qualify(bytes: &[u8], case: &model::Case) -> (Vec<String>, Vec<Vec<Value>>) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    // Admit the actual selected source without evaluating a frame or invoking
    // any writer. Metadata inspection alone is not this service's Source gate.
    let selectors = case
        .source
        .selection
        .iter()
        .map(|index| BeforeEventGuardSelector::new(3, &case.source.properties[*index].0))
        .collect::<Vec<_>>();
    let selected =
        SelectedPrograms::capture(&image, &schema, &selectors, &mut budget(), &|| false).unwrap();
    let qualified =
        preserved_source::capture_before_change(&image, "Items", &selected, &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(qualified.records.len(), 3);

    let names = schema
        .columns()
        .iter()
        .filter(|field| field.scalar_type().is_some())
        .map(|field| field.name())
        .collect::<Vec<_>>();
    let rows = read_table_projection(
        &mut image.reader(),
        SCOPE,
        "Items",
        &names,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let properties =
        inspect_table_properties(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    let original_properties = inspect_table_properties(
        &mut Cursor::new(&case.source.before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(properties.raw_bytes(), original_properties.raw_bytes());
    crate::constraints::check_scalar_projection(&rows, &properties, &mut budget(), &|| false)
        .unwrap();
    let coverage = image
        .inspect_table_macro_coverage(schema.object_id(), &mut budget(), &|| false)
        .unwrap();
    assert!(std::ptr::eq(coverage.image(), &image));
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    assert_eq!(
        coverage.table_extra().coverage(),
        TableExtraCoverage::WideMr2
    );
    assert_eq!(
        coverage.table_extra().raw(),
        Some(case.source.extra.as_slice())
    );
    let group = &coverage.table_extra().groups()[0];
    assert_eq!(group.kind(), 3);
    let programs = group.properties().unwrap();
    assert_eq!(programs.len(), case.source.properties.len());
    for (program, (name, xml)) in programs.iter().zip(&case.source.properties) {
        assert_eq!(program.name(), name);
        assert_eq!(program.text(), Some(xml.as_str()));
        assert_eq!((program.flags(), program.native_type()), (1, 12));
        program
            .inspect_complete_before_events(&mut budget(), &|| false)
            .unwrap();
    }
    let indexes =
        inspect_table_indexes(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(indexes.indexes().len(), 3);
    let metadata = Metadata::capture_for_preserved_boolean(
        &image,
        &schema,
        indexes.indexes(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    metadata
        .require_source(&image, schema.definition_page())
        .unwrap();
    let cached = read_stored_int32_calculation(
        &mut image.reader(),
        SCOPE,
        "Items",
        "Calculated",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(cached.values(), &[Some(2), Some(4), Some(6)]);
    let attachments = read_attachment_column(
        &mut image.reader(),
        SCOPE,
        "Items",
        "Attachment_Field",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        attachments
            .links()
            .parents()
            .iter()
            .map(|parent| parent.child_count())
            .collect::<Vec<_>>(),
        [2, 1, 0]
    );
    assert_eq!(image.as_bytes(), bytes);
    let names = rows
        .columns()
        .iter()
        .map(|field| field.name().to_owned())
        .collect();
    (names, rows.into_parts().2)
}

#[test]
fn qualify_literal_rich_before_change_batch_images_before_writer() {
    let mut images = 0;
    let mut refusals = 0;
    let mut changed_images = 0;
    let mut changed_roots = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            model::assert_complement(&case);
            let Some(expected) = &case.expected else {
                assert!(case.refusal.is_some());
                refusals += 1;
                continue;
            };
            assert_eq!(code, 6);
            let (names, mut final_rows) = qualify(&case.source.before, &case);
            for group in model::final_groups(&case) {
                let column = names.iter().position(|name| name == group.name).unwrap();
                for update in group.updates {
                    final_rows[update.row][column] = update.replacement.value();
                }
            }
            let (after_names, after_rows) = qualify(expected, &case);
            assert_eq!(after_names, names);
            assert_eq!(after_rows, final_rows);
            let rows = case
                .groups
                .iter()
                .flat_map(|group| group.updates.iter().map(|update| update.row))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            assert_eq!(case.event_rows, rows);
            images += 1;
            changed_images += usize::from(case.changed != 0);
            changed_roots += case.changed;
        }
    }
    assert_eq!(
        (images, refusals, changed_images, changed_roots),
        (15, 43, 12, 16)
    );
}
