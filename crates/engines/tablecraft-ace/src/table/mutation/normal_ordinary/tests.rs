//! Literal complete-image overlays over independent original content sources.
use super::proof;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan, NativeImage};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, LegacyDateTime, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_content::{self as model, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(114)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn first_record(bytes: &[u8], page: u32) -> usize {
    let base = page as usize * 4096;
    let word = u16::from_le_bytes(bytes[base + 14..base + 16].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    base + usize::from(word)
}

#[test]
fn ordinary_int32_changes_preserve_xml_other_tables_and_relocated_roles() {
    let mut images = 0;
    for header in [5, 6] {
        for case in [
            Case::Extrema,
            Case::Named,
            Case::ProgramLikeName,
            Case::XmlData,
            Case::Multiple,
            Case::ReorderedCatalog,
            Case::RelocatedUser,
            Case::RelocatedMaps,
            Case::SplitData,
            Case::TrailingFreePage,
        ] {
            let mut source = model::source(case);
            source.bytes[20] = header;
            let target = source
                .tables
                .iter()
                .find(|table| !table.rows.is_empty())
                .unwrap();
            let Value::Int32(before) = target.rows[0][0] else {
                panic!("independent first field must be Int32");
            };
            let updates = [FixedScalarUpdate::new(
                0,
                Value::Int32(before),
                Value::Int32(73),
            )];
            let groups = [FixedColumnUpdates::new(
                &target.name,
                &target.columns[0],
                &updates,
            )];
            let mut expected = source.bytes.clone();
            let start = first_record(&expected, target.data[0]);
            expected[start + 2..start + 6].copy_from_slice(&[73, 0, 0, 0]);

            let plan = image(&source.bytes)
                .plan_ordinary_fixed_updates(&target.name, &groups, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.before_bytes(), source.bytes, "{}", case.id());
            assert_eq!(plan.after_bytes(), expected, "{}", case.id());
            assert_eq!(plan.changed_rows(), 1);
            images += 1;
        }
    }
    assert_eq!(images, 20);
}

#[test]
fn ordinary_joint_wide_masks_null_inactive_bytes_and_exact_date_bits() {
    for header in [5, 6] {
        let mut source = model::source(Case::InactiveXml);
        source.bytes[20] = header;
        let flag = [FixedScalarUpdate::new(
            0,
            Value::Boolean(false),
            Value::Boolean(true),
        )];
        let wide = [FixedScalarUpdate::new(0, Value::Null, Value::Int64(7))];
        let money = [FixedScalarUpdate::new(
            0,
            Value::Currency(Currency::from_scaled(1)),
            Value::Currency(Currency::from_scaled(-10001)),
        )];
        let date = [FixedScalarUpdate::new(
            0,
            Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
            Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
        )];
        let groups = [
            FixedColumnUpdates::new("Items", "Flag", &flag),
            FixedColumnUpdates::new("Items", "Int64Value", &wide),
            FixedColumnUpdates::new("Items", "Money", &money),
            FixedColumnUpdates::new("Items", "DateValue", &date),
        ];
        let mut expected = source.bytes.clone();
        let start = first_record(&expected, 8);
        assert_eq!(&expected[start + 9..start + 17], b"<Event/>");
        expected[start + 9..start + 17].copy_from_slice(&7_i64.to_le_bytes());
        expected[start + 17..start + 25].copy_from_slice(&(-10001_i64).to_le_bytes());
        expected[start + 37..start + 45].copy_from_slice(&0x8000_0000_0000_0000_u64.to_le_bytes());
        expected[start + 61] |= 0x11;
        let plan = image(&source.bytes)
            .plan_ordinary_fixed_updates("Items", &groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.changed_rows(), 1);

        let noop = [FixedScalarUpdate::new(0, Value::Null, Value::Null)];
        let groups = [FixedColumnUpdates::new("Items", "Int64Value", &noop)];
        let plan = image(&source.bytes)
            .plan_ordinary_fixed_updates("Items", &groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), source.bytes);
        assert_eq!(plan.changed_rows(), 0);
    }
}

#[test]
fn original_preflight_and_fresh_proof_refuse_stale_duplicate_and_altered_bytes() {
    let source = model::source(Case::Extrema);
    let stale = [FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(73))];
    let groups = [FixedColumnUpdates::new("Items", "Id", &stale)];
    assert!(matches!(
        image(&source.bytes)
            .plan_ordinary_fixed_updates("Items", &groups, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));

    let updates = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(73))];
    let groups = [
        FixedColumnUpdates::new("Items", "Id", &updates),
        FixedColumnUpdates::new("Items", "ID", &updates),
    ];
    assert!(matches!(
        image(&source.bytes)
            .plan_ordinary_fixed_updates("Items", &groups, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));

    let groups = [FixedColumnUpdates::new("Items", "Id", &updates)];
    let mut expected = source.bytes.clone();
    let start = first_record(&expected, 8);
    expected[start + 2..start + 6].copy_from_slice(&[73, 0, 0, 0]);
    let mut observation = NativeEditPlan {
        before: source.bytes.clone(),
        after: expected,
        changed_rows: 1,
    };
    proof::prove(
        &observation,
        scope(),
        "Items",
        &groups,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    observation.after[start + 6] ^= 1;
    assert!(matches!(
        proof::prove(
            &observation,
            scope(),
            "Items",
            &groups,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    ));
    observation.after[start + 6] ^= 1;
    observation.changed_rows = 0;
    assert!(matches!(
        proof::prove(
            &observation,
            scope(),
            "Items",
            &groups,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    ));
}

#[test]
fn ordinary_empty_groups_and_present_noops_keep_all_original_bytes() {
    let source = model::source(Case::Sixteen);
    let target = &source.tables[0];
    let groups = [FixedColumnUpdates::new(
        &target.name,
        &target.columns[0],
        &[],
    )];
    let plan = image(&source.bytes)
        .plan_ordinary_fixed_updates(&target.name, &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), source.bytes);
    assert_eq!(plan.changed_rows(), 0);

    let source = model::source(Case::Extrema);
    let updates = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(1))];
    let groups = [FixedColumnUpdates::new("Items", "Id", &updates)];
    let plan = image(&source.bytes)
        .plan_ordinary_fixed_updates("Items", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), source.bytes);
    assert_eq!(plan.changed_rows(), 0);
}
