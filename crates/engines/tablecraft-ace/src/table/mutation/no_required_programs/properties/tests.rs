//! Real selected nonempty bags, not synthetic system catalog property grafts.
use super::capture;
use crate::{EditError, NativeImage, WriteBlocker, inspect_table_schema};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(132);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires two literal-hash-pinned genuine originals"]
fn original_sentinel_publish_zero_qualifies_but_unknown_code_keys_types_flags_and_values_refuse() {
    for source in crate::properties::framing::tests::originals() {
        let image = NativeImage::capture(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let schema = inspect_table_schema(
            &mut image.reader(),
            SCOPE,
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        capture(&image, &schema, &mut budget(), &|| false).unwrap();
        super::super::catalog::validate(&image, &schema, &mut budget(), &|| false)
            .expect("complete original catalog framing");
        super::super::declarations::capture(&image, &mut budget(), &|| false)
            .expect("complete original all-local LVAL declaration census");
        crate::table::mutation::ordinary_content::definition::validate_user(
            &image,
            &schema,
            &mut budget(),
            &|| false,
        )
        .expect("complete original selected TD and LVAL framing");
        // Root must compose the selected readonly LVAL declaration successor
        // before this source-only conjunction is executed. No writer is called.
        super::super::NoRequiredTablePrograms::capture(&image, &schema, &mut budget(), &|| false)
            .expect("complete original Sentinel program/property conjunction");
        image
            .check_native_ordinary_fixed_table("Sentinel", &mut budget(), &|| false)
            .expect("full original Source protection and actual fixed target");
        image
            .check_native_ordinary_fitted_table("Sentinel", &mut budget(), &|| false)
            .expect("full original Source protection and actual Text target");
        assert_eq!(image.as_bytes(), source.bytes);
        for control in 0..4 {
            let mut changed = source.bytes.clone();
            match control {
                0 => changed[source.property_offset + 12..source.property_offset + 36]
                    .copy_from_slice(&[
                        66, 0, 101, 0, 102, 0, 111, 0, 114, 0, 101, 0, 67, 0, 104, 0, 97, 0, 110,
                        0, 103, 0, 101, 0,
                    ]), // Same-width BeforeChange dictionary key is not passive metadata.
                1 => changed[source.property_offset + 51] = 0xfe,
                2 => changed[source.property_offset + 50] = 2,
                3 => changed[source.property_offset + 56] = 1,
                _ => unreachable!(),
            }
            let image =
                NativeImage::capture(&mut Cursor::new(&changed), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let schema = inspect_table_schema(
                &mut image.reader(),
                SCOPE,
                "Sentinel",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                capture(&image, &schema, &mut budget(), &|| false),
                Err(EditError::Unsupported(WriteBlocker::Properties)),
                "control {control}"
            );
        }
    }
}
