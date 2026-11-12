//! Known inert database settings preserve the full retained source certificate.
use crate::table::table_append::Inventory;
use crate::{
    DatabaseQueryMode, FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup,
    NativeFittedUpdates, NativeImage, NativeUpdateIntent, inspect_database_properties,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{fields, long_column_batch},
    database_properties as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(64109);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
}

fn source(properties: &[u8]) -> Vec<u8> {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    raw::with_native17_properties(&before, properties)
}

#[test]
fn original_name_tracking_prerequisites_and_exact_known_query_modes_qualify_independently() {
    for (mode, expected) in [
        (None, DatabaseQueryMode::Absent),
        (Some(0), DatabaseQueryMode::Ansi89),
        (Some(1), DatabaseQueryMode::Ansi92),
    ] {
        let properties = mode.map_or_else(
            raw::name_auto_correct_disabled,
            raw::name_auto_correct_disabled_with_query_mode,
        );
        let before = source(&properties);
        let native = image(&before);
        Inventory::capture(&native, &mut budget(), &|| false)
            .unwrap()
            .require_source(&native)
            .unwrap();
        let observed =
            inspect_database_properties(&mut native.reader(), SCOPE, &mut budget(), &|| false)
                .unwrap()
                .unwrap();
        assert_eq!(observed.raw_bytes(), properties);
        assert_eq!(
            observed.query_mode(&mut budget(), &|| false).unwrap(),
            expected
        );
        assert_eq!(observed.groups().len(), 1);
        assert_eq!(
            observed.groups()[0].properties().len(),
            if mode.is_some() { 3 } else { 2 }
        );
        assert_eq!(native.as_bytes(), before);
        assert!(Inventory::capture(&native, &mut budget(), &|| true).is_err());
        assert!(Inventory::capture(&native, &mut ResourceBudget::new(0, 0, 0), &|| false).is_err());
    }
}

#[test]
fn unknown_flags_types_widths_values_names_and_duplicate_mode_entries_stay_closed() {
    let zero = 0_i32.to_le_bytes();
    let one = 1_i32.to_le_bytes();
    let two = 2_i32.to_le_bytes();
    let negative = (-1_i32).to_le_bytes();
    let short = [0_u8];
    let wrong_kind = [0_u8; 2];
    for entries in [
        vec![(0, 4, zero.as_slice())],
        vec![(2, 4, zero.as_slice())],
        vec![(1, 3, wrong_kind.as_slice())],
        vec![(1, 4, short.as_slice())],
        vec![(1, 4, two.as_slice())],
        vec![(1, 4, negative.as_slice())],
        vec![(1, 4, zero.as_slice()), (1, 4, one.as_slice())],
    ] {
        let properties = raw::name_auto_correct_disabled_with_query_mode_values(&entries);
        let before = source(&properties);
        let native = image(&before);
        assert!(Inventory::capture(&native, &mut budget(), &|| false).is_err());
        assert_eq!(native.as_bytes(), before);
    }
    let mut unknown = raw::name_auto_correct_disabled_with_query_mode(0);
    let original = "ANSI Query Mode"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let replacement = "ANSI Query M0de"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let positions = unknown
        .windows(original.len())
        .enumerate()
        .filter_map(|(at, value)| (value == original).then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    let at = positions[0];
    unknown[at..at + replacement.len()].copy_from_slice(&replacement);
    let before = source(&unknown);
    assert!(Inventory::capture(&image(&before), &mut budget(), &|| false).is_err());
}

#[test]
fn a_mode_property_never_replaces_or_relaxes_required_name_tracking_metadata() {
    let zero = 0_i32.to_le_bytes();
    let one = 1_i32.to_le_bytes();
    for properties in [
        raw::query_modes(&[(1, 4, &zero)]),
        raw::name_auto_correct_values(&[(1, 4, &one), (1, 4, &zero)]),
        raw::name_auto_correct_values(&[(1, 4, &zero), (0, 4, &zero)]),
        raw::name_auto_correct_values(&[(1, 4, &zero)]),
    ] {
        let before = source(&properties);
        assert!(Inventory::capture(&image(&before), &mut budget(), &|| false).is_err());
    }
}

#[test]
fn existing_long_originals_preserve_mode_bytes_and_bind_the_complete_original_source() {
    let case = long_column_batch::two_rows();
    for mode in [0, 1] {
        let properties = raw::name_auto_correct_disabled_with_query_mode(mode);
        let before = raw::with_native17_properties(&case.before, &properties);
        let expected = raw::with_native17_properties(&case.expected, &properties);
        let groups = vec![
            NativeFittedIntentGroup::new_retained_long_column_batch(
                case.column,
                NativeFittedUpdates::Fixed(
                    case.cells
                        .iter()
                        .map(|cell| {
                            FixedScalarUpdate::new(
                                cell.row,
                                cell.expected.map_or(Value::Null, Value::Int32),
                                cell.value.map_or(Value::Null, Value::Int32),
                            )
                        })
                        .collect(),
                ),
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        ];
        let plan = image(&before)
            .plan_recoverable_retained_long_column_batch_updates(
                case.table,
                groups,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected);
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(&wire[..8], b"TCNI0025");
        assert!(NativeFittedEditIntent::decode_retained(&wire, &mut budget(), &|| false).is_err());
        let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        let changed_mode = raw::name_auto_correct_disabled_with_query_mode(1 - mode);
        let changed_source = raw::with_native17_properties(&case.before, &changed_mode);
        assert!(
            decoded
                .verify_before(&changed_source, &mut budget(), &|| false)
                .is_err()
        );
        let after = image(plan.after_bytes());
        Inventory::capture(&after, &mut budget(), &|| false).unwrap();
        let observed =
            inspect_database_properties(&mut after.reader(), SCOPE, &mut budget(), &|| false)
                .unwrap()
                .unwrap();
        assert_eq!(observed.raw_bytes(), properties);
    }
}
