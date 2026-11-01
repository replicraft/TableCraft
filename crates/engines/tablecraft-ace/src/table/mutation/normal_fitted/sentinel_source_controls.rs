//! Original-only Source and target-binding controls; never stage or plan writes.
use super::binding;
use crate::{
    EditError, FittedColumnUpdates, FixedColumnUpdates, NativeImage, WriteBlocker,
    inspect_table_schema,
    table::mutation::{
        fixed_records,
        no_required_programs::NoRequiredTablePrograms,
        preserved_source::{self, Source},
        record_serialization::storage,
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(134);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn capture(image: &NativeImage) -> Result<Source, EditError> {
    let schema = inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )?;
    let certificate = NoRequiredTablePrograms::capture(image, &schema, &mut budget(), &|| false)?;
    preserved_source::capture_no_required_programs(
        image,
        "Sentinel",
        &certificate,
        &mut budget(),
        &|| false,
    )
}

#[test]
#[ignore = "requires the unchanged two hash-pinned genuine Sentinel originals"]
fn selected_sentinel_properties_lval_and_marker_bind_without_mutation() {
    for original in crate::properties::framing::tests::originals() {
        let image = image(&original.bytes);
        let source = capture(&image).unwrap();
        assert_eq!(
            source.table.rows(),
            &[vec![
                Value::Int32(23),
                Value::Text("unchanged sentinel".into()),
                Value::Binary(vec![199, 0, 255]),
            ]]
        );
        assert_eq!(source.table.schema().columns()[2].native_type(), 11);
        let marker = [FittedColumnUpdates::Variable {
            column: "Marker",
            updates: &[],
        }];
        binding::all(&source, &marker, &mut budget(), &|| false).unwrap();
        let bytes = [FittedColumnUpdates::Variable {
            column: "Bytes",
            updates: &[],
        }];
        assert!(matches!(
            binding::all(&source, &bytes, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        let bytes_fixed = [FixedColumnUpdates::new("Sentinel", "Bytes", &[])];
        assert!(matches!(
            fixed_records::binding::all_normal(&source, &bytes_fixed, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        assert_eq!(image.as_bytes(), original.bytes);
    }
}

#[test]
#[ignore = "requires the unchanged two hash-pinned genuine Sentinel originals"]
fn malformed_selected_lval_pairs_fail_full_source_not_just_pair_count() {
    for original in crate::properties::framing::tests::originals() {
        let header = original.bytes[20];
        let td = (if header == 5 { 107 } else { 125 }) * PAGE;
        let pair = td + 170;
        for variant in 0..7 {
            let mut bytes = original.bytes.clone();
            match variant {
                0 => bytes[pair..pair + 2].copy_from_slice(&[0, 0]),
                1 => bytes[pair..pair + 2].copy_from_slice(&[255, 255]),
                2 => bytes[pair + 2..pair + 6].fill(0),
                3 => bytes[pair + 6..pair + 10].fill(0),
                4 => bytes[pair + 2..pair + 6].copy_from_slice(&0xffff_ff00_u32.to_le_bytes()),
                5 => bytes[pair + 6..pair + 10].copy_from_slice(&0xffff_ff00_u32.to_le_bytes()),
                6 => {
                    // A real nonzero but aliased physical map record. Entry
                    // count and field number remain correct; actual Source
                    // must deny aliasing its selected ordinary-owned map.
                    let owned = original.bytes[td + 55..td + 59].to_vec();
                    bytes[pair + 2..pair + 6].copy_from_slice(&owned);
                }
                _ => unreachable!(),
            }
            let image = image(&bytes);
            assert!(
                capture(&image).is_err(),
                "header {header}, variant {variant}"
            );
            assert_eq!(image.as_bytes(), bytes);
        }
    }
}

#[test]
#[ignore = "requires the unchanged two hash-pinned genuine Sentinel originals"]
fn memo_metadata_cannot_become_a_fitted_target_through_scalar_text_projection() {
    for original in crate::properties::framing::tests::originals() {
        let td = (if original.bytes[20] == 5 { 107 } else { 125 }) * PAGE;
        let mut bytes = original.bytes.clone();
        let field = td + 63 + 2 * 25;
        bytes[field] = 12;
        bytes[field + 11..field + 15].copy_from_slice(&[9, 4, 0, 1]);
        let image = image(&bytes);
        let schema = inspect_table_schema(
            &mut image.reader(),
            SCOPE,
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.columns()[2].native_type(), 12);
        assert_eq!(
            storage::qualify(&schema.columns()[2]),
            Err(EditError::Unsupported(WriteBlocker::Column))
        );
        // This is a metadata-only target control. Retyped OLE bytes are not a
        // healthy Memo source, and no fake projected Source is constructed.
        assert_eq!(image.as_bytes(), bytes);
    }
}
