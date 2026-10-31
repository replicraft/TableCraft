//! Additive healthy Required originals; no mutation expectation or writer.
use super::super::fixed_scalar_sources::model as scalar_model;
use super::model;

use crate::{
    NativeImage, PropertyGroupKind, inspect_table_indexes, inspect_table_properties,
    inspect_table_schema, read_table_projection,
    table::mutation::variable_split::metadata::Metadata,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn bag(bytes: &[u8]) -> (usize, &[u8]) {
    let starts = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(at, prefix)| (prefix == b"MR2\0").then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(starts.len(), 1);
    let start = starts[0];
    assert!(start >= 12);
    let envelope = u32::from_le_bytes(bytes[start - 12..start - 8].try_into().unwrap());
    assert_eq!(envelope & 0xc000_0000, 0x8000_0000);
    assert_eq!(&bytes[start - 8..start], &[0; 8]);
    let length = usize::try_from(envelope & 0x3fff_ffff).unwrap();
    (start, &bytes[start..start + length])
}

fn patch_matching_bag(bytes: &mut [u8], original: &[u8], relative: usize) -> usize {
    let starts = bytes
        .windows(original.len())
        .enumerate()
        .filter_map(|(at, value)| (value == original).then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(starts.len(), 1);
    let patch = starts[0] + relative;
    assert_eq!(bytes[patch], 0);
    bytes[patch] = 1;
    patch
}

pub(crate) struct Case {
    pub source: model::Case,
    pub patch: usize,
    pub original_patch: usize,
    pub repairing: bool,
}

pub(crate) fn source(code: u8, repairing: bool) -> Case {
    let mut source = model::source(
        code,
        if repairing {
            model::Recipe::Int32DerivedOnly
        } else {
            model::Recipe::DormantDelete
        },
    );
    let authority = scalar_model::source(
        code,
        scalar_model::Target::LargeNumber,
        scalar_model::Recipe::RequiredRefusal,
    );
    let (authority_start, required_bag) = bag(&authority.before);
    let (_, original_bag) = bag(&source.original);
    let original_bag = original_bag.to_vec();
    let relative = authority
        .required_patch
        .unwrap()
        .checked_sub(authority_start)
        .unwrap();
    assert_eq!(required_bag.len(), original_bag.len());
    assert_eq!(&required_bag[..relative], &original_bag[..relative]);
    assert_eq!(&required_bag[relative + 1..], &original_bag[relative + 1..]);
    assert_eq!((original_bag[relative], required_bag[relative]), (0, 1));
    // The actual existing helper selects Large_Number's Required entry. Check
    // its raw boolean framing here, and independently re-read the named field
    // with the normal property inspector in the SOURCE control below.
    assert_eq!(
        &original_bag[relative - 8..relative],
        &[9, 0, 1, 1, 10, 0, 1, 0]
    );
    assert_eq!(word(&original_bag, relative - 4), 10);
    let before = source.before.clone();
    let original = source.original.clone();
    let patch = patch_matching_bag(&mut source.before, &original_bag, relative);
    let original_patch = patch_matching_bag(&mut source.original, &original_bag, relative);
    for (old, new, changed) in [
        (&before, &source.before, patch),
        (&original, &source.original, original_patch),
    ] {
        assert_eq!(&old[..changed], &new[..changed]);
        assert_eq!(&old[changed + 1..], &new[changed + 1..]);
    }
    Case {
        source,
        patch,
        original_patch,
        repairing,
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn qualify_rich_before_change_required_originals_before_expected_or_writer() {
    let scope = CatalogScope::new(1);
    let mut originals = 0;
    for code in [5, 6] {
        for repairing in [true, false] {
            let case = source(code, repairing);
            assert_eq!(case.repairing, repairing);
            assert_eq!(case.source.before[case.patch], 1);
            assert_eq!(case.source.original[case.original_patch], 1);
            let image = NativeImage::capture(
                &mut Cursor::new(&case.source.before),
                scope,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let schema =
                inspect_table_schema(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(schema.columns()[4].name(), "Large_Number");
            assert_eq!(schema.columns()[4].native_type(), 4);
            assert_eq!(schema.columns()[4].storage_length(), 4);
            let properties = inspect_table_properties(
                &mut image.reader(),
                scope,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                properties
                    .group(PropertyGroupKind::Column, "Large_Number")
                    .unwrap()
                    .get("Required")
                    .unwrap()
                    .boolean(),
                Ok(true)
            );
            let original_properties = inspect_table_properties(
                &mut Cursor::new(&case.source.original),
                scope,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(properties.raw_bytes(), original_properties.raw_bytes());
            let names = schema
                .columns()
                .iter()
                .filter(|field| field.scalar_type().is_some())
                .map(|field| field.name())
                .collect::<Vec<_>>();
            let rows = read_table_projection(
                &mut image.reader(),
                scope,
                "Items",
                &names,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let selected = rows
                .columns()
                .iter()
                .position(|field| field.name() == "Large_Number")
                .unwrap();
            assert_eq!(
                rows.rows()
                    .iter()
                    .map(|row| &row[selected])
                    .collect::<Vec<_>>(),
                [
                    &Value::Int32(1000),
                    &Value::Int32(2000),
                    &Value::Int32(3000)
                ]
            );
            crate::constraints::check_scalar_projection(&rows, &properties, &mut budget(), &|| {
                false
            })
            .unwrap();
            let coverage = image
                .inspect_table_macro_coverage(schema.object_id(), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                coverage.table_extra().raw(),
                Some(case.source.extra.as_slice())
            );
            assert!(coverage.children().is_empty());
            assert_eq!(coverage.body().presence(), crate::PayloadPresence::Absent);
            assert_eq!(coverage.module().presence(), crate::PayloadPresence::Absent);
            let indexes =
                inspect_table_indexes(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
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
            assert_eq!(image.as_bytes(), case.source.before);
            originals += 1;
        }
    }
    assert_eq!(originals, 4);
}
