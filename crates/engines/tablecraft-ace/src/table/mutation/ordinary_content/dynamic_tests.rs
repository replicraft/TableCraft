//! Source-only closure checks precede the independent mutation consumers.
use super::capture;
use crate::{NativeImage, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_inline as sources;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn dynamic_inline_sources_close_ten_complete_images_and_refuse_fourteen_controls() {
    let scope = CatalogScope::new(116);
    let mut accepted = 0;
    let mut refused = 0;
    for header in [5, 6] {
        for case in sources::CASES {
            let mut source = sources::source(case);
            source.bytes[20] = header;
            assert_eq!(source.bytes.len(), 96 * 4096);
            let image = NativeImage::capture(
                &mut Cursor::new(&source.bytes),
                scope,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let result = capture(&image, &mut budget(), &|| false);
            if !source.ordinary {
                assert!(result.is_err(), "{}", case.id());
                refused += 1;
                continue;
            }
            let certificate = result.unwrap_or_else(|error| panic!("{}: {error:?}", case.id()));
            let table = read_table(
                &mut image.reader(),
                scope,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let expected = source
                .rows
                .iter()
                .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            assert_eq!(table.rows(), expected);
            certificate
                .require_source(&image, table.schema(), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(image.as_bytes(), source.bytes);
            accepted += 1;
        }
    }
    assert_eq!(accepted, 10);
    assert_eq!(refused, 14);
}

// Construct only the new adversarial physical facts, independently of the
// production map/record parsers and without changing the frozen SOURCE cases.
fn allocated_high_data_without_global_cells() -> Vec<u8> {
    let mut source = sources::source(sources::Case::MissingGlobalCoverage);
    assert_eq!(source.data, 95);
    assert_eq!(source.rows, [vec![1, 10]]);
    assert_eq!(source.global.len(), 13);
    assert_eq!(&source.global[..5], &[0; 5]);

    let container = 3 * 4096;
    for slot in [3, 7] {
        let word = container + 14 + 2 * slot;
        let start = usize::from(u16::from_le_bytes(
            source.bytes[word..word + 2].try_into().unwrap(),
        ));
        let end = usize::from(u16::from_le_bytes(
            source.bytes[word - 2..word].try_into().unwrap(),
        ));
        assert_eq!(start & 0xe000, 0);
        assert_eq!(end - start, 9);
        let record = &mut source.bytes[container + start..container + end];
        assert_eq!(&record[..5], &[0, 64, 0, 0, 0]);
        // All 32 pages64..95 are owned, and all have candidate membership.
        record[5..].fill(0xff);
    }

    for page in 64..95 {
        let start = page * 4096;
        let data = &mut source.bytes[start..start + 4096];
        assert!(data.iter().all(|byte| *byte == 0));
        data[..2].copy_from_slice(&[1, 1]);
        data[2..4].copy_from_slice(&4082_u16.to_le_bytes());
        data[4..8].copy_from_slice(&7_u32.to_le_bytes());
        // Slot count0, all reserved bytes and the full 4082-byte free region
        // remain zero. This is a complete empty DATA page for actual TD7.
    }
    source.bytes
}

#[test]
fn missing_global_cells_refuse_even_when_every_high_page_has_a_real_role() {
    let scope = CatalogScope::new(117);
    for header in [5, 6] {
        let mut bytes = allocated_high_data_without_global_cells();
        bytes[20] = header;
        let image = NativeImage::capture(&mut Cursor::new(&bytes), scope, &mut budget(), &|| false)
            .unwrap();
        // The ordinary native reader sees all 32 actual owned DATA pages,
        // including the 31 correctly framed empty pages, and the original row.
        let table = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(table.rows(), [vec![Value::Int32(1), Value::Int32(10)]]);
        assert!(capture(&image, &mut budget(), &|| false).is_err());
        assert_eq!(image.as_bytes(), bytes);
    }
}
