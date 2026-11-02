//! Actual raw declaration consumers use original independent source bytes.
use super::validate_user;
use crate::{EditError, InspectError, NativeImage, inspect_table_schema};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_creation as plain,
    canonical_ordinary_indexed::{self as indexed, Case},
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(124);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn raw_user_declarations_share_plain_counter_alias_and_tree_source_laws() {
    for header in [5, 6] {
        for case in plain::CASES {
            let mut source = plain::source(case);
            source.bytes[20] = header;
            let image = image(&source.bytes);
            let schema = inspect_table_schema(
                &mut image.reader(),
                SCOPE,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            validate_user(&image, &schema, &mut budget(), &|| false).unwrap();
            assert_eq!(image.as_bytes(), source.bytes);
        }
        for case in indexed::CASES {
            let mut source = indexed::source(case);
            if !source.ordinary {
                continue;
            }
            source.bytes[20] = header;
            let image = image(&source.bytes);
            let schema = inspect_table_schema(
                &mut image.reader(),
                SCOPE,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            validate_user(&image, &schema, &mut budget(), &|| false).unwrap();
            assert_eq!(image.as_bytes(), source.bytes);
        }
    }
}

#[test]
fn raw_user_declarations_refuse_unclassified_header_descriptor_count_and_tail_bytes() {
    for header in [5, 6] {
        let original = indexed::source(Case::AliasHistory);
        let td = 7 * PAGE;
        // This actual original has one physical index and two aliases. Compute
        // independent raw section positions from literal wire declarations.
        let physical =
            u32::from_le_bytes(original.bytes[td + 51..td + 55].try_into().unwrap()) as usize;
        let logical =
            u32::from_le_bytes(original.bytes[td + 47..td + 51].try_into().unwrap()) as usize;
        let count =
            u16::from_le_bytes(original.bytes[td + 45..td + 47].try_into().unwrap()) as usize;
        assert_eq!((physical, logical, count), (1, 2, 2));
        let columns = td + 63 + physical * 12;
        let mut names_end = columns + count * 25;
        for _ in 0..count {
            let length =
                u16::from_le_bytes(original.bytes[names_end..names_end + 2].try_into().unwrap())
                    as usize;
            names_end += 2 + length;
        }
        let logical_start = names_end + physical * 52;
        let length =
            u32::from_le_bytes(original.bytes[td + 8..td + 12].try_into().unwrap()) as usize;
        let controls = [
            td + 28,            // Reserved TD header byte.
            td + 63,            // Reserved count-block prefix.
            td + 63 + 8,        // Reserved count-block suffix.
            columns + 17,       // Reserved native column byte.
            names_end + 42,     // Reserved physical descriptor byte.
            logical_start + 12, // Reserved logical descriptor byte.
            td + length - 1,    // Exact terminal FFFF.
            td + length,        // Undeclared TD tail byte.
        ];
        for at in controls {
            let mut bytes = original.bytes.clone();
            bytes[20] = header;
            bytes[at] ^= 1;
            let image = image(&bytes);
            let schema = inspect_table_schema(
                &mut image.reader(),
                SCOPE,
                original.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                validate_user(&image, &schema, &mut budget(), &|| false).is_err(),
                "{at}"
            );
            assert_eq!(image.as_bytes(), bytes);
        }
    }
}

#[test]
fn selected_raw_declaration_validation_pays_all_work_and_can_cancel_before_return() {
    for header in [5, 6] {
        let mut source = indexed::source(Case::AliasHistory);
        source.bytes[20] = header;
        let image = image(&source.bytes);
        let schema = inspect_table_schema(
            &mut image.reader(),
            SCOPE,
            source.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut measured = budget();
        let initial = POOLS.map(|resource| measured.remaining(resource));
        validate_user(&image, &schema, &mut measured, &|| false).unwrap();
        let costs: [u64; 3] =
            std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        // This operation inspects metadata without scanning user rows.
        assert!(costs[0] > 0 && costs[2] > 0);
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        validate_user(&image, &schema, &mut exact, &|| false).unwrap();
        for resource in POOLS {
            assert_eq!(exact.remaining(resource), 0);
        }
        for (index, resource) in POOLS.into_iter().enumerate() {
            if costs[index] == 0 {
                continue;
            }
            let mut limits = costs;
            limits[index] -= 1;
            let error = validate_user(
                &image,
                &schema,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false,
            )
            .unwrap_err();
            assert!(
                matches!(error, EditError::Inspect(InspectError::Limit(error)) if error.resource == resource)
            );
        }
        let polls = Cell::new(0_usize);
        validate_user(&image, &schema, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let terminal = polls.get();
        for stop in [1, terminal / 2, terminal] {
            polls.set(0);
            let error = validate_user(&image, &schema, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            })
            .unwrap_err();
            assert_eq!(error, EditError::Inspect(InspectError::Cancelled));
            assert_eq!(polls.get(), stop);
        }
        assert_eq!(image.as_bytes(), source.bytes);
    }
}

#[test]
#[ignore = "requires the two pinned genuine original files; no producer is invoked"]
fn pinned_genuine_user_declarations_classify_three_fields_and_four_logical_aliases() {
    let root = std::env::var_os("TABLECRAFT_GENUINE_ORDINARY_SOURCE_DIR")
        .expect("root must explicitly provide the retained source directory");
    let root = Path::new(&root);
    assert!(root.is_absolute());
    for (filename, length, digest, definition) in [
        (
            "h5-distinct.accdb",
            405504,
            [
                0x72, 0xe7, 0xba, 0x54, 0x80, 0xd9, 0x5d, 0x0b, 0xe0, 0xd4, 0xea, 0x6f, 0x2c, 0x75,
                0xb0, 0x15, 0x06, 0x29, 0x70, 0x59, 0x9d, 0xa3, 0xfd, 0xe6, 0x20, 0xcc, 0x9f, 0x13,
                0xc6, 0x0b, 0x02, 0x8d,
            ],
            89,
        ),
        (
            "h6-distinct.accdb",
            475136,
            [
                0x5c, 0xfb, 0xf6, 0xd4, 0xb6, 0x22, 0x4b, 0x98, 0x9f, 0x00, 0xf0, 0xab, 0xd6, 0x3e,
                0xc3, 0x62, 0x84, 0x6f, 0xf6, 0x02, 0x40, 0xa9, 0x1b, 0x05, 0x8e, 0x14, 0x42, 0xc3,
                0x01, 0x03, 0x05, 0xc5,
            ],
            106,
        ),
    ] {
        let bytes = std::fs::read(root.join(filename)).unwrap();
        assert_eq!(bytes.len(), length);
        let actual = Sha256::digest(&bytes);
        assert_eq!(&actual[..], &digest);
        let image = image(&bytes);
        let schema =
            inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(schema.definition_page(), definition);
        assert_eq!(schema.columns().len(), 3);
        assert_eq!(schema.layout.physical_indexes, 2);
        assert_eq!(schema.layout.logical_indexes, 4);
        validate_user(&image, &schema, &mut budget(), &|| false).unwrap();
        assert_eq!(image.as_bytes(), bytes);
    }
}
