//! A numeric schema certificate is readonly and cannot widen old action sources.
use super::{SCOPE, allowance, image, same_bytes};
use crate::table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary};
use crate::table::table_append::Inventory;
use crate::{EditError, SecondaryIndexCreate, WriteBlocker};
use tablecraft_testkit::fixtures::access::secondary_index::numeric::{
    self as raw, Kind, Population,
};

#[test]
fn numeric_schema_capture_keeps_old_schema_and_action_factories_closed() {
    for kind in [Kind::Boolean, Kind::Single, Kind::Double, Kind::DateTime] {
        let case = raw::source(kind, Population::NullableDuplicate, false);
        let mut work = allowance(case.rows.len());
        let after = image(&case.expected, &mut work);
        assert!(matches!(
            normal_ordinary::capture_retained_schema(&after, case.table, &mut work, &|| false),
            Err(EditError::Unsupported(WriteBlocker::ActiveContent))
        ));
        let schema =
            crate::inspect_table_schema(&mut after.reader(), SCOPE, case.table, &mut work, &|| {
                false
            })
            .unwrap();
        let inventory = Inventory::capture(&after, &mut work, &|| false).unwrap();
        let certificate = NoRequiredTablePrograms::capture_retained_numeric_index_schema(
            &after,
            &schema,
            &inventory,
            &mut work,
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            certificate.require_retained_schema_source(&after, &schema, &mut work, &|| false,),
            Err(EditError::Preservation)
        ));
        let readonly = normal_ordinary::capture_retained_numeric_index_schema(
            &after,
            case.table,
            &inventory,
            &mut work,
            &|| false,
        )
        .unwrap();
        assert_eq!(readonly.table.rows().len(), case.rows.len());
        assert_eq!(readonly.indexes.indexes().len(), 2);
        same_bytes(after.as_bytes(), &case.expected);
    }
}

#[test]
fn subsequent_numeric_creation_requalifies_existing_numeric_indexes_without_retry() {
    for kind in [Kind::Boolean, Kind::Single, Kind::Double, Kind::DateTime] {
        let case = raw::source(kind, Population::NullableDuplicate, true);
        let mut work = allowance(case.rows.len());
        let created = image(&case.expected, &mut work)
            .prepare_retained_numeric_index_creation(
                case.table,
                &SecondaryIndexCreate::new("AmountLookup", "Amount", false),
                &mut work,
                &|| false,
            )
            .unwrap();
        assert_eq!(created.logical_index_number(), 2);
        assert_eq!(created.physical_index_number(), 2);
        assert_eq!(created.native_number(), 2);
        assert_eq!(created.indexed_rows(), case.rows.len());
        same_bytes(created.before_bytes(), &case.expected);
        for number in &case.index_pages {
            let start = *number as usize * 4096;
            same_bytes(
                &created.candidate_bytes()[start..start + 4096],
                &case.expected[start..start + 4096],
            );
        }
    }
}

#[test]
fn numeric_content_cannot_reuse_a_certificate_from_an_equal_but_distinct_image() {
    let case = raw::source(Kind::Double, Population::NullableDuplicate, false);
    let mut work = allowance(case.rows.len());
    let first = image(&case.expected, &mut work);
    let second = image(&case.expected, &mut work);
    let inventory = Inventory::capture(&first, &mut work, &|| false).unwrap();
    assert!(matches!(
        normal_ordinary::capture_retained_numeric_index_schema(
            &second,
            case.table,
            &inventory,
            &mut work,
            &|| false,
        ),
        Err(EditError::Preservation)
    ));
    assert!(matches!(
        normal_ordinary::capture_retained_numeric_index_schema(
            &first,
            case.table,
            &inventory,
            &mut work,
            &|| true,
        ),
        Err(EditError::Inspect(crate::InspectError::Cancelled))
    ));
    same_bytes(first.as_bytes(), &case.expected);
    same_bytes(second.as_bytes(), &case.expected);
}
