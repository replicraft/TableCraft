//! Read-only qualification of normal BeforeChange originals and literal outcomes.
use crate::{
    NativeImage, PayloadPresence, TableExtraCoverage, check_table, check_table_constraints,
    read_table, table::mutation::variable_split::metadata::Metadata,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_before_change::{self as model, Case};

const SCOPE: CatalogScope = CatalogScope::new(19);
const ORIGINAL: [[i32; 2]; 3] = [[1, 10], [2, 20], [3, 30]];

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 100_000_000)
}

fn qualify(case: &Case, bytes: &[u8], rows: &[[i32; 2]; 3]) {
    assert_eq!(bytes.len(), 36_864);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(checked.indexes().is_empty());
    check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    let expected = rows
        .iter()
        .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(read.rows(), expected);
    assert_eq!(read.schema().object_id(), SCOPE.object_id(7));
    assert_eq!(read.schema().columns().len(), 2);
    for (field, name) in read.schema().columns().iter().zip(["Id", "Score"]) {
        assert_eq!(field.name(), name);
        assert_eq!(
            (
                field.native_type(),
                field.flags(),
                field.extended_flags(),
                field.storage_length()
            ),
            (4, 3, 0, 4)
        );
        assert!(field.fixed());
    }
    let coverage = image
        .inspect_table_macro_coverage(SCOPE.object_id(7), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        coverage.body().presence(),
        if case.body_present {
            PayloadPresence::Present
        } else {
            PayloadPresence::Absent
        }
    );
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    let extra = coverage.table_extra();
    assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
    assert_eq!(extra.raw(), Some(case.extra.as_slice()));
    assert_eq!((extra.native_id(), extra.parent_id()), (7, 0x0f00_0001));
    assert_eq!((extra.catalog_page(), extra.catalog_slot()), (4, 7));
    let protected = Metadata::capture_for_insert(
        &image,
        checked.schema(),
        checked.indexes(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    protected.require_source(&image, 7).unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_normal_before_change_sources_and_complete_expectations_before_writer() {
    let mut originals = 0;
    let mut expected = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            qualify(&case, &case.before, &ORIGINAL);
            originals += 1;
            if let Some(after) = &case.expected {
                qualify(&case, after, &case.final_rows.unwrap());
                expected += 1;
            }
        }
    }
    assert_eq!(originals, 32);
    assert_eq!(expected, 14);
}

mod export;

mod hash_anchors;
