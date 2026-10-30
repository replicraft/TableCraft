//! Additive selected-table orphan witness; no mutation/key coverage is inferred.
use super::model::{self, Family, PAGE};
use std::io::Cursor;
use tablecraft_ace::{NativeImage, check_table, inspect_table_indexes, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PHYSICAL_START: usize = 143;
const PHYSICAL_END: usize = PHYSICAL_START + 52;

pub(super) fn source(code: u8) -> Vec<u8> {
    let mut source = model::case(code, Family::SameTable).before;
    assert_eq!(
        &source[10 * PAGE + 47..10 * PAGE + 51],
        &2_u32.to_le_bytes()
    );
    assert_eq!(
        &source[10 * PAGE + 51..10 * PAGE + 55],
        &1_u32.to_le_bytes()
    );
    source[10 * PAGE + 47..10 * PAGE + 51].copy_from_slice(&0_u32.to_le_bytes());
    // Complete empty trailing declaration list follows the physical descriptor.
    // The abandoned logical records/names remain unparsed opaque source bytes.
    source[10 * PAGE + PHYSICAL_END..10 * PAGE + PHYSICAL_END + 2].copy_from_slice(&[255, 255]);
    source
}

#[test]
fn qualify_selected_physical_orphan_source_before_maintenance_probe() {
    for code in [1, 5, 6] {
        let bytes = source(code);
        let scope = CatalogScope::new(1);
        let mut budget = ResourceBudget::new(128 * 1024 * 1024, 100_000, 100_000_000);
        let image =
            NativeImage::capture(&mut Cursor::new(&bytes), scope, &mut budget, &|| false).unwrap();
        assert_eq!(image.as_bytes(), bytes);
        let inspection =
            inspect_table_indexes(&mut image.reader(), scope, "Items", &mut budget, &|| false)
                .unwrap();
        assert!(inspection.indexes().is_empty());
        let checked =
            check_table(&mut image.reader(), scope, "Items", &mut budget, &|| false).unwrap();
        assert_eq!(checked.rows(), 3);
        assert!(checked.indexes().is_empty());
        let table =
            read_table(&mut image.reader(), scope, "Items", &mut budget, &|| false).unwrap();
        assert_eq!(
            table.rows(),
            &[
                vec![Value::Int32(1), Value::Int32(10)],
                vec![Value::Int32(2), Value::Int32(20)],
                vec![Value::Int32(3), Value::Int32(30)],
            ]
        );
        assert_eq!(&bytes[10 * PAGE + 51..10 * PAGE + 55], &1_u32.to_le_bytes());
        assert_eq!(
            &bytes[10 * PAGE + PHYSICAL_START + 4..10 * PAGE + PHYSICAL_START + 6],
            &1_u16.to_le_bytes()
        );
        assert_eq!(
            &bytes[10 * PAGE + PHYSICAL_START + 34..10 * PAGE + PHYSICAL_START + 38],
            &(14_u32 << 8).to_le_bytes()
        );
        assert_eq!(
            &bytes[10 * PAGE + PHYSICAL_START + 38..10 * PAGE + PHYSICAL_START + 42],
            &13_u32.to_le_bytes()
        );
        let map = model::row_start(&bytes, 14, 0);
        assert_eq!(&bytes[map..map + 7], &[0, 0, 0, 0, 0, 0, 0x20]);
        assert_eq!(&bytes[13 * PAGE..13 * PAGE + 2], &[4, 1]);
        assert_eq!(&bytes[13 * PAGE + 4..13 * PAGE + 8], &10_u32.to_le_bytes());
        assert_eq!(
            &bytes[13 * PAGE + 480..13 * PAGE + 489],
            &[0x7f, 0x80, 0, 0, 10, 0, 0, 12, 0]
        );
        // There is one explicit physical key owner, but no logical maintenance
        // profile. This is a read/source fact, not permission to modify its keys.
    }
}
