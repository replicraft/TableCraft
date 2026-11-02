//! Exact native IEEE observations before any generic scalar creation call.
use super::gates;
use crate::{NativeImage, read_table, table::mutation::variable_split::metadata::Metadata};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation, canonical_fixed_float_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "Explicit fresh export of independent IEEE sources; no production creator"]
fn export_fixed_ieee_sources_for_read_only_comparison() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_FIXED_FLOAT_SOURCE_DIR")
            .expect("an explicit fresh IEEE source directory is required"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();

    for case in canonical_fixed_float_creation::CASES {
        let source = canonical_fixed_float_creation::source(case);
        let path = directory.join(format!("fixed-ieee-h5-{}.accdb", case.id()));
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        output.write_all(&source.bytes).unwrap();
        output.flush().unwrap();
        assert_eq!(fs::read(path).unwrap(), source.bytes);
    }
}

#[test]
fn qualify_fixed_ieee_sources_before_generic_creator() {
    for case in canonical_fixed_float_creation::CASES {
        let source = canonical_fixed_float_creation::source(case);
        let base = canonical_fixed_creation::source(canonical_fixed_creation::Case::SignedZero);
        assert_eq!(source.bytes.len(), base.bytes.len());
        assert_eq!(source.rows.len(), 2);
        let first = 8 * 4096 + 4096 - 63;
        let second = 8 * 4096 + 4096 - 2 * 63;
        let allowed = [
            first + 25..first + 29,
            first + 29..first + 37,
            second + 25..second + 29,
            second + 29..second + 37,
        ];
        assert_eq!(allowed.iter().map(|span| span.len()).sum::<usize>(), 24);
        for offset in 0..source.bytes.len() {
            if !allowed.iter().any(|span| span.contains(&offset)) {
                assert_eq!(source.bytes[offset], base.bytes[offset]);
            }
        }
        for (row, (single, double)) in canonical_fixed_float_creation::float_bits(case)
            .into_iter()
            .enumerate()
        {
            let start = 8 * 4096 + 4096 - (row + 1) * 63;
            assert_eq!(&source.bytes[start + 25..start + 29], &single.to_le_bytes());
            assert_eq!(&source.bytes[start + 29..start + 37], &double.to_le_bytes());
        }

        let image = NativeImage::capture(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut resources = budget();
        let read = read_table(
            &mut image.reader(),
            SCOPE,
            source.table,
            &mut resources,
            &|| false,
        )
        .unwrap();
        // Shared Value equality is representational, including NaN payloads,
        // signed zeros and signalling bits. Numeric/temporal equality would
        // erase precisely the facts this SOURCE continuation is qualifying.
        assert_eq!(read.rows(), source.rows, "case {}", case.id());
        let structural = gates::before(&image, source.table, &mut resources, &|| false).unwrap();
        let metadata = Metadata::capture_for_insert(
            &image,
            structural.schema(),
            structural.indexes(),
            &mut resources,
            &|| false,
        )
        .unwrap();
        metadata
            .require_source(&image, structural.schema().definition_page())
            .unwrap();
        assert_eq!(image.as_bytes(), source.bytes);
    }
}

mod hash_anchor;
