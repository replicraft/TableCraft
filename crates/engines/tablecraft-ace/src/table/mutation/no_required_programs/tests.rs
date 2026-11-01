//! Original-only controls: no planner, evaluator, candidate or writer is called.
use super::NoRequiredTablePrograms;
use crate::{NativeImage, TableSchema, inspect_table_schema};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(127);
const CARRIERS: [usize; 9] = [8, 9, 10, 11, 12, 13, 14, 15, 16];
// Independent SDK/raw observations, not native reader geometry constants.
const PAGE: usize = 4096;
const CATALOG_PAGE: usize = 17;
const DIRECTORY_START: usize = 14;
const DIRECTORY_WORD: usize = 2;
const ROW_OFFSET_MASK: u16 = 0x1fff;

struct Original {
    filename: &'static str,
    digest: &'static str,
    definition: u32,
    items_slot: u8,
    items_start: usize,
    items_end: usize,
    sentinel_slot: u8,
    sentinel_start: usize,
    sentinel_end: usize,
}

const ORIGINALS: [Original; 2] = [
    Original {
        filename: "h5-distinct.accdb",
        digest: "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
        definition: 89,
        items_slot: 34,
        items_start: 503,
        items_end: 576,
        sentinel_slot: 35,
        sentinel_start: 424,
        sentinel_end: 503,
    },
    Original {
        filename: "h6-distinct.accdb",
        digest: "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
        definition: 106,
        items_slot: 39,
        items_start: 213,
        items_end: 286,
        sentinel_slot: 40,
        sentinel_start: 134,
        sentinel_end: 213,
    },
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> Result<NativeImage, crate::InspectError> {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
}

fn schema(image: &NativeImage, name: &str) -> TableSchema {
    inspect_table_schema(&mut image.reader(), SCOPE, name, &mut budget(), &|| false).unwrap()
}

fn refused(bytes: &[u8]) {
    // A malformed control may fail the original reader before certificate
    // issuance. Neither outcome may be reclassified as a write permission.
    let Ok(original) = image(bytes) else {
        return;
    };
    let Ok(target) = inspect_table_schema(
        &mut original.reader(),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    ) else {
        return;
    };
    assert!(
        NoRequiredTablePrograms::capture(&original, &target, &mut budget(), &|| false).is_err()
    );
}

/// The raw source has Name at 32..42, Owner at 42..44 and nine empty variables
/// at 44. Shortening Owner to 42..43 lets one NULL variable retain byte 43 without
/// resizing the row or changing the native catalog identity/name/NULL mask.
fn null_carriers_with_hidden_bytes_refuse(bytes: &[u8], source: &Original) {
    let page = CATALOG_PAGE * PAGE;
    let row = page + source.items_start;
    let end = page + source.items_end;
    let variable_count = end - 5;
    assert_eq!(&bytes[variable_count..variable_count + 2], &[11, 0]);
    assert_eq!(end - row, 73);
    assert_eq!(&bytes[end - 3..end], &[0xff, 0, 0]);

    // Independent footer words are stored in reverse variable-index order.
    for index in 0..12 {
        let at = variable_count - 2 * (index + 1);
        let expected = match index {
            0 => 32,
            1 => 42,
            _ => 44,
        };
        assert_eq!(
            u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()),
            expected,
        );
    }

    for (field, variable) in (8_usize..=16).zip(2_usize..=10) {
        let mut control = bytes.to_vec();
        // Move every boundary from Owner's end through this variable's start.
        // Earlier NULL spans remain empty; this variable alone becomes 43..44.
        for index in 2..=variable {
            let at = variable_count - 2 * (index + 1);
            control[at..at + 2].copy_from_slice(&43_u16.to_le_bytes());
        }
        let offsets: [u16; 12] = std::array::from_fn(|index| {
            let at = variable_count - 2 * (index + 1);
            u16::from_le_bytes(control[at..at + 2].try_into().unwrap())
        });
        assert!(offsets.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!((offsets[variable], offsets[variable + 1]), (43, 44));
        assert_eq!(&control[row..row + 44], &bytes[row..row + 44]);
        assert_eq!(&control[end - 3..end], &[0xff, 0, 0]);
        assert_eq!(control[end - 3 + field / 8] & (1 << (field % 8)), 0);

        // This must reach the strict validator. A native capture/schema error
        // would fail the test rather than count as the intended refusal.
        let original = image(&control).expect("valid catalog row with a hidden NULL span");
        let target = schema(&original, "Items");
        assert_eq!(target.definition_page(), source.definition);
        let error = super::catalog::validate(&original, &target, &mut budget(), &|| false)
            .expect_err("a NULL bit cannot hide nonempty carrier bytes");
        assert_eq!(
            error,
            crate::EditError::Unsupported(crate::WriteBlocker::ActiveContent),
            "native field {field}",
        );
        assert_eq!(original.as_bytes(), control);
    }
}

#[test]
#[ignore = "requires the two explicit immutable genuine original files"]
fn genuine_originals_bind_and_all_nine_present_empty_carriers_refuse() {
    let directory = std::env::var_os("TABLECRAFT_GENUINE_NO_REQUIRED_SOURCES_DIR").unwrap();
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());

    for source in ORIGINALS {
        let bytes = std::fs::read(directory.join(source.filename)).unwrap();
        let actual = Sha256::digest(&bytes);
        let actual = actual
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, source.digest);
        let original = image(&bytes).unwrap();
        let target = schema(&original, "Items");
        assert_eq!(target.definition_page(), source.definition);
        super::catalog::validate(&original, &target, &mut budget(), &|| false)
            .expect("complete original catalog profile");
        super::declarations::capture(&original, &mut budget(), &|| false)
            .expect("complete original LVAL declaration coverage");
        crate::table::mutation::ordinary_content::definition::validate_user(
            &original,
            &target,
            &mut budget(),
            &|| false,
        )
        .expect("complete original selected definition");
        let certificate =
            NoRequiredTablePrograms::capture(&original, &target, &mut budget(), &|| false).unwrap();
        certificate
            .require_source(&original, &target, &mut budget(), &|| false)
            .unwrap();
        original
            .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
            .expect("full original Source protection and actual target binding");
        let separately_captured = image(&bytes).unwrap();
        assert!(
            certificate
                .require_source(&separately_captured, &target, &mut budget(), &|| false)
                .is_err()
        );

        let origin = original
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == target.object_id())
            .unwrap();
        assert_eq!(origin.page(), CATALOG_PAGE as u32);
        assert_eq!(origin.slot(), source.items_slot);
        let page_start = CATALOG_PAGE * PAGE;
        let start = source.items_start;
        let end = source.items_end;
        let directory_word =
            page_start + DIRECTORY_START + DIRECTORY_WORD * usize::from(source.items_slot);
        assert_eq!(
            u16::from_le_bytes(
                bytes[directory_word..directory_word + 2]
                    .try_into()
                    .unwrap()
            ),
            start as u16,
        );
        assert_eq!(
            u16::from_le_bytes(
                bytes[directory_word - 2..directory_word]
                    .try_into()
                    .unwrap()
            ) & ROW_OFFSET_MASK,
            end as u16,
        );
        assert_eq!(&bytes[page_start + start..page_start + start + 2], &[17, 0]);
        let mask = page_start + end - 3;
        assert_eq!(&bytes[mask..mask + 3], &[0xff, 0, 0]);

        // The independently observed original has all nine NULL cells and no
        // corresponding stored bytes. Setting only presence makes present-empty,
        // which must refuse even if public three-stream coverage looks inert.
        for field in CARRIERS {
            assert_eq!(bytes[mask + field / 8] & (1 << (field % 8)), 0);
            let mut changed = bytes.clone();
            changed[mask + field / 8] |= 1 << (field % 8);
            refused(&changed);
        }

        null_carriers_with_hidden_bytes_refuse(&bytes, &source);

        let mut short = bytes.clone();
        short[page_start + start..page_start + start + 2].copy_from_slice(&16_u16.to_le_bytes());
        refused(&short);

        let mut deleted = bytes.clone();
        let at = directory_word;
        let entry = u16::from_le_bytes(deleted[at..at + 2].try_into().unwrap());
        deleted[at..at + 2].copy_from_slice(&(entry | 0x8000).to_le_bytes());
        refused(&deleted);

        let sentinel = schema(&original, "Sentinel");
        let sentinel_origin = original
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == sentinel.object_id())
            .unwrap();
        assert_eq!(sentinel_origin.page(), CATALOG_PAGE as u32);
        assert_eq!(sentinel_origin.slot(), source.sentinel_slot);
        let sentinel_word =
            page_start + DIRECTORY_START + DIRECTORY_WORD * usize::from(source.sentinel_slot);
        assert_eq!(
            u16::from_le_bytes(bytes[sentinel_word..sentinel_word + 2].try_into().unwrap()),
            source.sentinel_start as u16,
        );
        assert_eq!(
            u16::from_le_bytes(bytes[sentinel_word - 2..sentinel_word].try_into().unwrap())
                & ROW_OFFSET_MASK,
            source.sentinel_end as u16,
        );
        assert_eq!(source.sentinel_end, source.items_start);
        let sentinel_row = page_start + source.sentinel_start;
        let mut child = bytes.clone();
        child[sentinel_row + 6..sentinel_row + 10]
            .copy_from_slice(&target.definition_page().to_le_bytes());
        refused(&child);
        child[sentinel_row + 6..sentinel_row + 10]
            .copy_from_slice(&(0x0100_0000 | target.definition_page()).to_le_bytes());
        refused(&child);

        let mut cross_kind_alias = bytes.clone();
        cross_kind_alias[sentinel_row + 2..sentinel_row + 6]
            .copy_from_slice(&(0x0100_0000 | target.definition_page()).to_le_bytes());
        cross_kind_alias[sentinel_row + 10..sentinel_row + 12]
            .copy_from_slice(&3_u16.to_le_bytes());
        refused(&cross_kind_alias);

        for kind in [0_i16, 6] {
            let mut unsupported_storage = bytes.clone();
            unsupported_storage[sentinel_row + 10..sentinel_row + 12]
                .copy_from_slice(&kind.to_le_bytes());
            refused(&unsupported_storage);
        }

        let mut reserved = bytes.clone();
        reserved[source.definition as usize * PAGE + 25] ^= 1;
        refused(&reserved);
        assert_eq!(original.as_bytes(), bytes);
        if source.definition == 106 {
            // The pinned h6 native directory contains exactly this empty
            // deleted+relocated slot. Its preceding endpoint is also 980.
            let tombstone = 17 * PAGE + 14 + 2 * 32;
            let word = u16::from_le_bytes(bytes[tombstone..tombstone + 2].try_into().unwrap());
            assert_eq!(word, 0xc3d4);
            assert_eq!(word & 0x1fff, 980);
            let prior = u16::from_le_bytes(bytes[tombstone - 2..tombstone].try_into().unwrap());
            assert_eq!(prior & 0x1fff, 980);

            let mut unsupported_tombstone = bytes.clone();
            unsupported_tombstone[tombstone..tombstone + 2]
                .copy_from_slice(&(word | 0x2000).to_le_bytes());
            refused(&unsupported_tombstone);

            // Keep both original flags but give the slot one byte of body.
            // A reader refusal is valid; a certificate must never be issued.
            let mut nonempty_tombstone = bytes.clone();
            nonempty_tombstone[tombstone..tombstone + 2].copy_from_slice(&0xc3d3_u16.to_le_bytes());
            refused(&nonempty_tombstone);
        }
    }
}
