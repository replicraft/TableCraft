//! Existing independent images reused as read-only ordinary IEEE UPDATE pairs.
use crate::{
    NativeImage, check_table, inspect_table_schema, read_table,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation as declarations, canonical_ordinary_wide_insert as sources,
};

const SCOPE: CatalogScope = CatalogScope::new(262);
const PAGE: usize = 4096;
const DATA: usize = 8;
const ROW_BYTES: usize = 63;
const SELECTED_START: usize = DATA * PAGE + 3907;
const PINS: &[(u8, &str, &str)] = &[
    (
        5,
        "exact",
        "b04d36341ed843e3bf7f2b9bfabd790e9a075d1b8002ad5a53253968c3ea82ed",
    ),
    (
        5,
        "subnormal",
        "8b3cd0ceeda4a9d3875bc8d2d5e2c786097dec4b6b06484ec4ffeb09dce72476",
    ),
    (
        5,
        "special",
        "168914cbc9325e2670576150eddbd23b150f1d1f795a53bdf48c63d642660019",
    ),
    (
        6,
        "exact",
        "1375d5e7453d0603ed42226b05c2f7afa6280f88d63287e2ab73b334a0da339f",
    ),
    (
        6,
        "subnormal",
        "323b381d5484715a0929f29d8451c3a1c2c0c63d02ac9a1978c1148ce713741d",
    ),
    (
        6,
        "special",
        "464904879eaf0b5e70e2a645cac80486497f1b0235cf81fb8e7ae1f1ceb28232",
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn bits(recipe: &str) -> (u32, u64, u64) {
    match recipe {
        "exact" => (0x8000_0000, 0x8000_0000_0000_0000, 0x8000_0000_0000_0000),
        "subnormal" => (1, 0x000f_ffff_ffff_ffff, 0),
        "special" => (0x7f81_2345, 0xfff8_1234_5678_9abc, 0),
        _ => panic!("outside the three existing independent recipes"),
    }
}

fn expected_rows(recipe: &str) -> Vec<Vec<Value>> {
    let case = match recipe {
        "exact" => sources::Case::FixedExact,
        "subnormal" => sources::Case::FixedSubnormal,
        "special" => sources::Case::FixedSpecial,
        _ => panic!("outside the three existing independent recipes"),
    };
    // Reuse the existing independently authored fixture's semantic rows. This
    // does not call a native writer, export images or define a second fixture.
    let source = sources::source(case);
    let mut rows = source.rows_before;
    rows.push(source.values);
    rows
}

fn assert_record(raw: &[u8], recipe: &str) {
    assert_eq!(raw.len(), 9 * PAGE);
    assert_eq!(&raw[7 * PAGE + 16..7 * PAGE + 20], &3_u32.to_le_bytes());
    assert_eq!(&raw[DATA * PAGE..DATA * PAGE + 2], &[1, 1]);
    assert_eq!(&raw[DATA * PAGE + 4..DATA * PAGE + 8], &7_u32.to_le_bytes());
    assert_eq!(
        &raw[DATA * PAGE + 12..DATA * PAGE + 14],
        &3_u16.to_le_bytes()
    );
    assert_eq!(
        &raw[DATA * PAGE + 2..DATA * PAGE + 4],
        &3887_u16.to_le_bytes()
    );
    for slot in 0..3 {
        let offset = DATA * PAGE + 14 + 2 * slot;
        assert_eq!(
            &raw[offset..offset + 2],
            &u16::try_from(PAGE - ROW_BYTES * (slot + 1))
                .unwrap()
                .to_le_bytes(),
        );
    }

    let record = &raw[SELECTED_START..SELECTED_START + ROW_BYTES];
    let (single, double, date) = bits(recipe);
    assert_eq!(&record[..2], &10_u16.to_le_bytes());
    assert_eq!(&record[25..29], &single.to_le_bytes());
    assert_eq!(&record[29..37], &double.to_le_bytes());
    assert_eq!(&record[37..45], &date.to_le_bytes());
    assert_eq!(&record[61..63], &[0xff, 3]);
}

fn assert_pair(before: &[u8], after: &[u8], recipe: &str) {
    let (single, double, date) = bits(recipe);
    let mut expected = before.to_vec();
    expected[SELECTED_START + 25..SELECTED_START + 29].copy_from_slice(&single.to_le_bytes());
    expected[SELECTED_START + 29..SELECTED_START + 37].copy_from_slice(&double.to_le_bytes());
    if recipe == "subnormal" {
        expected[SELECTED_START + 37..SELECTED_START + 45].copy_from_slice(&date.to_le_bytes());
    }
    // Equality checks the entire complement, including counts/maps, all other
    // rows, fixed siblings, original masks, raw slack, catalog and EOF.
    assert_eq!(expected, after);
    assert_ne!(before, after);
}

#[test]
#[ignore = "explicit six pinned existing sources; no mutable carrier admission or writer"]
fn existing_ieee_pairs_read_exact_bits_and_observe_current_source_admission() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_IEEE_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());

    // Pin every complete image before any source API is called.
    let images: Vec<_> = PINS
        .iter()
        .map(|&(header, recipe, expected_hash)| {
            let raw =
                fs::read(directory.join(format!("h{header}-fixed-{recipe}-after.accdb"))).unwrap();
            assert_eq!(raw.len(), 36864);
            let actual_hash: String = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(actual_hash, expected_hash);
            assert_eq!(&raw[20..24], &u32::from(header).to_le_bytes());
            (header, recipe, raw)
        })
        .collect();

    for (header, recipe, raw) in &images {
        assert_record(raw, recipe);
        let expected = expected_rows(recipe);
        assert_eq!(expected.len(), 3);
        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
        let schema =
            inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(schema.definition_page(), 7);
        assert_eq!(schema.columns().len(), declarations::COLUMNS.len());
        for (number, (actual, literal)) in schema
            .columns()
            .iter()
            .zip(&declarations::COLUMNS)
            .enumerate()
        {
            assert_eq!(actual.number(), number);
            assert_eq!(actual.name(), literal.name);
            assert_eq!(actual.native_type(), literal.native_type);
            assert_eq!(actual.storage_length(), usize::from(literal.width));
            assert!(actual.fixed());
            assert_eq!(actual.flags(), 3);
            assert_eq!(actual.extended_flags(), 0);
        }

        let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(read.rows(), expected);
        let (single, double, date) = bits(recipe);
        assert!(matches!(&read.rows()[2][6], Value::Single(value) if value.to_bits() == single));
        assert!(matches!(&read.rows()[2][7], Value::Double(value) if value.to_bits() == double));
        assert!(matches!(&read.rows()[2][8], Value::DateTime(value) if value.to_bits() == date));

        let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(checked.rows(), 3);
        assert!(checked.indexes().is_empty());

        match NoRequiredTablePrograms::capture_updates(&image, &schema, &mut budget(), &|| false) {
            Ok(_certificate) => {
                eprintln!("IEEE_NO_REQUIRED header={header} recipe={recipe} ADMITTED")
            }
            Err(error) => {
                eprintln!("IEEE_NO_REQUIRED header={header} recipe={recipe} REFUSED {error:?}")
            }
        }
        match normal_ordinary::capture(&image, "Items", &mut budget(), &|| false) {
            Ok(source) => {
                assert_eq!(source.table.rows(), expected);
                assert_eq!(source.records.len(), 3);
                for (slot, record) in source.records.iter().enumerate() {
                    assert_eq!(record.reference, ((DATA as u32) << 8) | slot as u32);
                    let start = DATA * PAGE + PAGE - ROW_BYTES * (slot + 1);
                    assert_eq!(record.data, raw[start..start + ROW_BYTES]);
                }
                eprintln!("IEEE_CANONICAL_SOURCE header={header} recipe={recipe} ADMITTED");
            }
            Err(error) => {
                eprintln!("IEEE_CANONICAL_SOURCE header={header} recipe={recipe} REFUSED {error:?}")
            }
        }
        assert_eq!(image.as_bytes(), raw);
    }

    let mut pairs = 0;
    for header in [5, 6] {
        let lookup = |name| {
            &images
                .iter()
                .find(|(h, recipe, _)| *h == header && *recipe == name)
                .unwrap()
                .2
        };
        assert_pair(lookup("exact"), lookup("subnormal"), "subnormal");
        assert_pair(lookup("subnormal"), lookup("special"), "special");
        pairs += 2;
    }
    assert_eq!((images.len(), pairs), (6, 4));
}
