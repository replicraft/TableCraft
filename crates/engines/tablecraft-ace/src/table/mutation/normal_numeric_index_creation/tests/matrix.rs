//! Independent raw declarations, complete INDEX images and native field identities.
use super::{allowance, image, same_bytes};
use crate::SecondaryIndexCreate;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::secondary_index::numeric::{
    self as raw, Kind, Population,
};

fn scalar(kind: Kind) -> ScalarType {
    match kind {
        Kind::Boolean => ScalarType::Boolean,
        Kind::Byte => ScalarType::Byte,
        Kind::Int16 => ScalarType::Int16,
        Kind::Int32 => ScalarType::Int32,
        Kind::Int64 => ScalarType::Int64,
        Kind::Currency => ScalarType::Currency,
        Kind::Single => ScalarType::Single,
        Kind::Double => ScalarType::Double,
        Kind::DateTime => ScalarType::DateTime,
    }
}

fn compare(case: &raw::Source) {
    let mut work = allowance(case.rows.len());
    let candidate = image(&case.before, &mut work)
        .prepare_retained_numeric_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, case.descending),
            &mut work,
            &|| false,
        )
        .unwrap_or_else(|error| {
            panic!(
                "kind={:?}, rows={}, descending={}: {error:?}",
                case.kind,
                case.rows.len(),
                case.descending
            )
        });
    same_bytes(candidate.before_bytes(), &case.before);
    same_bytes(candidate.candidate_bytes(), &case.expected);
    assert_eq!(candidate.scalar_type(), scalar(case.kind));
    assert_eq!(candidate.native_number(), 0);
    assert_eq!(candidate.definition_page(), case.target_definition);
    assert_eq!(candidate.descending(), case.descending);
    assert_eq!(candidate.logical_index_number(), 1);
    assert_eq!(candidate.physical_index_number(), 1);
    assert_eq!(candidate.indexed_rows(), case.rows.len());
    assert_eq!(candidate.root_page(), case.root);
    assert_eq!(candidate.index_pages(), case.index_pages.len());
    assert_eq!(usize::from(candidate.height()) + 1, case.levels);
}

#[test]
fn all_numeric_declarations_match_complete_literal_images_and_protect_existing_indexes() {
    for kind in raw::KINDS {
        for population in [Population::Empty, Population::NullableDuplicate] {
            for descending in [false, true] {
                compare(&raw::source(kind, population, descending));
            }
        }
    }
}

#[test]
fn eight_byte_population_matches_literal_multileaf_and_multilevel_topology() {
    for population in [Population::ThreeLeaves, Population::MultiLevel] {
        for descending in [false, true] {
            let case = raw::source(Kind::Int64, population, descending);
            assert!(case.leaf_pages > 2);
            if matches!(population, Population::MultiLevel) {
                assert!(case.levels >= 3);
            }
            compare(&case);
        }
    }
}

fn first_fixed_payload(case: &raw::Source) -> usize {
    let row = case.rows.iter().find(|row| row.value.is_some()).unwrap();
    let page = (row.reference >> 8) as usize * 4096;
    let slot = (row.reference & 255) as usize;
    assert_eq!(
        u32::from_le_bytes(case.before[page + 4..page + 8].try_into().unwrap()),
        case.target_definition
    );
    let directory = page + 14 + slot * 2;
    let offset = u16::from_le_bytes(case.before[directory..directory + 2].try_into().unwrap());
    assert_eq!(
        offset & 0xe000,
        0,
        "literal source must use a direct live record"
    );
    // The raw schema deliberately places ordinary native field0 at fixed
    // offset0, after the original two-byte stored field count.
    page + usize::from(offset & 0x1fff) + 2
}

#[test]
fn nonfinite_ieee_source_payloads_do_not_acquire_numeric_creation_authority() {
    for (kind, bits) in [
        (Kind::Single, 0x7f80_0000_u64),
        (Kind::Single, 0xff80_0000_u64),
        (Kind::Single, 0x7fc0_0001_u64),
        (Kind::Double, 0x7ff0_0000_0000_0000_u64),
        (Kind::Double, 0xfff0_0000_0000_0000_u64),
        (Kind::Double, 0x7ff8_0000_0000_0001_u64),
    ] {
        let mut case = raw::source(kind, Population::NullableDuplicate, false);
        let start = first_fixed_payload(&case);
        let width = if matches!(kind, Kind::Single) { 4 } else { 8 };
        case.before[start..start + width].copy_from_slice(&bits.to_le_bytes()[..width]);
        let mut work = allowance(case.rows.len());
        assert!(
            matches!(
                image(&case.before, &mut work).prepare_retained_numeric_index_creation(
                    case.table,
                    &SecondaryIndexCreate::new(case.index_name, case.column, false),
                    &mut work,
                    &|| false,
                ),
                Err(crate::EditError::Unsupported(crate::WriteBlocker::Column))
            ),
            "{kind:?} bits={bits:x}"
        );
    }
}
