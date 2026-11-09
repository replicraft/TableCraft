use super::*;
use crate::{NativeImage, inspect_table_properties};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::{Source, repeated};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn properties() -> TableProperties {
    let first = repeated::source(Source::IdOnly);
    let bytes = repeated::append(&first, "Score2");
    let scope = CatalogScope::new(982);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap();
    inspect_table_properties(&mut image.reader(), scope, "Table1", &mut budget(), &|| {
        false
    })
    .unwrap()
}

#[test]
fn group_visitor_uses_exact_raw_names_and_retains_default_dictionary_and_work() {
    let properties = properties();
    let mut ordinary = budget();
    let old = ordered_names(&properties, &mut ordinary, &|| false).unwrap();
    let mut observed = budget();
    let mut selected = None;
    let new = ordered_names_with_groups(&properties, &mut observed, &|| false, |index, span| {
        if index == 0 {
            selected = Some(span);
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(old, new);
    let span = selected.unwrap();
    assert_eq!(&properties.raw_bytes()[span], b"S\0c\0o\0r\0e\0");
    assert_eq!(
        ordinary.remaining(Resource::Bytes),
        observed.remaining(Resource::Bytes)
    );
    assert_eq!(
        ordinary.remaining(Resource::Steps) - observed.remaining(Resource::Steps),
        properties.groups().len() as u64
    );
}

#[test]
fn later_property_value_mismatch_discards_completed_framing_and_dictionary() {
    let mut properties = properties();
    let last = properties.raw.len() - 1;
    properties.raw[last] ^= 1;
    let mut visited = false;
    let result = ordered_names_with_groups(&properties, &mut budget(), &|| false, |_, _| {
        visited = true;
        Ok(())
    });
    assert!(visited);
    assert!(matches!(result, Err(InspectError::Unsupported)));
}
