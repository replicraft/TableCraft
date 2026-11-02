//! Literal original-source controls; no new successful image or writer oracle.
use crate::{
    EditError, NativeImage, PropertyGroupKind, RowInsert, WriteBlocker, inspect_table_properties,
    read_table,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(157);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

fn healthy(raw: &[u8]) {
    let original = image(raw);
    original
        .check_native_ordinary_fitted_table("Sentinel", &mut budget(), &|| false)
        .unwrap();
    let snapshot = read_table(
        &mut original.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        snapshot.rows(),
        [vec![
            Value::Int32(23),
            Value::Text("unchanged sentinel".into()),
            Value::Binary(vec![199, 0, 255])
        ]]
    );
    assert_eq!(original.as_bytes(), raw);
}

#[test]
#[ignore = "requires all four qualified original-only INSERT source files"]
fn whole_batch_exhausts_original_byte_capacity_before_publishing_any_candidate() {
    for case in cases() {
        healthy(&case.before);
        let base = case.data as usize * 4096;
        assert_eq!(&case.before[base + 2..base + 4], &[174, 15]);
        // Every independently qualified row is21B plus2B for its live slot.
        // 174*23=4002 fits4014; 175*23=4025 exceeds it. This is a refusal
        // witness, not an unqualified new successful174-row expected image.
        let free = usize::from(u16::from_le_bytes(
            case.before[base + 2..base + 4].try_into().unwrap(),
        ));
        assert!(174 * 23 <= free);
        assert!(175 * 23 > free);
        let requests = (0..175)
            .map(|_| RowInsert::new(sources::values()))
            .collect::<Vec<_>>();
        let saved = case.before.clone();
        let error = image(&case.before)
            .plan_ordinary_row_inserts("Sentinel", &requests, &mut budget(), &|| false)
            .err()
            .expect("last row cannot reuse an earlier shadow reservation");
        assert_eq!(error, EditError::Unsupported(WriteBlocker::RowLayout));
        assert_eq!(case.before, saved);
    }
}

#[test]
#[ignore = "requires all four qualified original-only INSERT source files"]
fn healthy_full_slot_directory_refuses_even_when_record_byte_capacity_remains() {
    for case in cases() {
        let mut saturated = case.before.clone();
        let base = case.data as usize * 4096;
        assert_eq!(&saturated[base + 14..base + 16], &[190, 15]);
        // Root0 and its66B payload remain exact. Slots1..254 are explicitly
        // deleted empty extents at4030, leaving3506B but no new u8 slot.
        saturated[base + 12..base + 14].copy_from_slice(&255_u16.to_le_bytes());
        saturated[base + 2..base + 4].copy_from_slice(&3506_u16.to_le_bytes());
        for slot in 1..255 {
            saturated[base + 14 + slot * 2..base + 16 + slot * 2]
                .copy_from_slice(&0x8fbe_u16.to_le_bytes());
        }
        assert_eq!(
            &saturated[base + 4030..base + 4096],
            &case.before[base + 4030..base + 4096]
        );
        healthy(&saturated);
        let saved = saturated.clone();
        let requests = [RowInsert::new(sources::values())];
        let error = image(&saturated)
            .plan_ordinary_row_inserts("Sentinel", &requests, &mut budget(), &|| false)
            .err()
            .expect("unused record capacity cannot manufacture slot255");
        assert_eq!(error, EditError::Unsupported(WriteBlocker::RowLayout));
        assert_eq!(saturated, saved);
    }
}

#[test]
#[ignore = "requires all four qualified original-only INSERT source files"]
fn explicit_empty_text_requires_its_real_insert_declaration() {
    for case in cases() {
        healthy(&case.before);
        let original = image(&case.before);
        let properties = inspect_table_properties(
            &mut original.reader(),
            SCOPE,
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let group = properties.group(PropertyGroupKind::Column, "Marker");
        assert!(
            group
                .and_then(|group| group.get("AllowZeroLength"))
                .is_none()
        );
        let requests = [RowInsert::new(vec![
            Value::Int32(24),
            Value::Text(String::new()),
            Value::Null,
        ])];
        let error = image(&case.before)
            .plan_ordinary_row_inserts("Sentinel", &requests, &mut budget(), &|| false)
            .err()
            .expect("missing INSERT policy cannot mean allowed zero-length Text");
        assert_eq!(
            error,
            EditError::Unsupported(WriteBlocker::ConstraintCoverage)
        );
    }
}
