use super::{budget, literal};
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeFixedEditIntent, NativeFixedIntentGroup,
    NativeImage, NativeIntentError, NativeUpdateIntent, RecoverableEditError, WriteBlocker,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_fixed_before_change as sources;

#[test]
fn fixed_group_metadata_refuses_duplicate_rows_without_reordering_distinct_requests() {
    for second_value in [Value::Int64(2), Value::Int64(3)] {
        let requests = vec![
            FixedScalarUpdate::new(2, Value::Int64(1), Value::Int64(2)),
            FixedScalarUpdate::new(0, Value::Int64(1), Value::Int64(2)),
            FixedScalarUpdate::new(2, Value::Int64(1), second_value),
        ];
        assert!(matches!(
            NativeFixedIntentGroup::new("I64", requests, &mut budget(), &|| false),
            Err(NativeIntentError::Request(EditError::Conflict))
        ));
    }
    let requests = vec![
        FixedScalarUpdate::new(2, Value::Int64(1), Value::Int64(2)),
        FixedScalarUpdate::new(0, Value::Int64(1), Value::Int64(1)),
    ];
    let original_pointer = requests.as_ptr();
    let group =
        NativeFixedIntentGroup::new("KeEpSpelling", requests, &mut budget(), &|| false).unwrap();
    assert_eq!(group.column(), "KeEpSpelling");
    assert_eq!(group.updates().as_ptr(), original_pointer);
    assert_eq!(
        group
            .updates()
            .iter()
            .map(FixedScalarUpdate::row)
            .collect::<Vec<_>>(),
        [2, 0]
    );
    assert_eq!(group.updates()[1].expected(), group.updates()[1].value());
}

#[test]
fn fixed_intent_metadata_refuses_case_variant_groups_even_when_empty() {
    for second_name in ["Money", "mOnEy"] {
        let groups = vec![
            NativeFixedIntentGroup::new("Money", Vec::new(), &mut budget(), &|| false).unwrap(),
            NativeFixedIntentGroup::new(second_name, Vec::new(), &mut budget(), &|| false).unwrap(),
        ];
        assert!(matches!(
            NativeFixedEditIntent::new("Items", groups, b"original", &mut budget(), &|| false),
            Err(NativeIntentError::Request(EditError::Conflict))
        ));
    }
    // Different columns may name the same row. Original order/spellings and the
    // complete independent 355-byte wire remain unchanged, with no Rows pool.
    let raw = literal();
    let mut metadata_only = ResourceBudget::new(16_000_000, 0, 16_000_000);
    let decoded = NativeUpdateIntent::decode(&raw, &mut metadata_only, &|| false).unwrap();
    let fixed = decoded.fixed().unwrap();
    assert_eq!(fixed.groups()[0].updates()[0].row(), 0);
    assert_eq!(fixed.groups()[3].updates()[0].row(), 0);
    assert_eq!(fixed.groups()[4].updates()[0].row(), 0);
    assert_eq!(decoded.encode(&mut metadata_only, &|| false).unwrap(), raw);
    assert_eq!(metadata_only.remaining(Resource::Rows), 0);
}

#[test]
fn well_framed_fixed_wires_cannot_encode_duplicate_request_identities() {
    let mut duplicate_row = literal();
    // Independent wire geometry: the I64 group starts at 160, name at 172,
    // first row word at 175 and second row word at 201. Both become ordinal 1.
    duplicate_row[201..209].copy_from_slice(&1_u64.to_le_bytes());
    assert!(matches!(
        NativeUpdateIntent::decode(&duplicate_row, &mut budget(), &|| false),
        Err(NativeIntentError::Request(EditError::Conflict))
    ));
    let mut duplicate_column = literal();
    // Same byte length as Date, so framing/counts/carriers remain well formed.
    duplicate_column[282..286].copy_from_slice(b"fLaG");
    assert!(matches!(
        NativeUpdateIntent::decode(&duplicate_column, &mut budget(), &|| false),
        Err(NativeIntentError::Request(EditError::Conflict))
    ));
}

#[test]
fn native_duplicate_column_preflight_priority_is_unchanged_by_intent_metadata() {
    for header in [5, 6] {
        let case = sources::source(header, sources::Recipe::DuplicateColumn);
        let groups = case
            .groups
            .iter()
            .map(|group| {
                let name = sources::FIELDS
                    .iter()
                    .find(|field| field.0 == group.column)
                    .unwrap()
                    .1;
                let requests = group
                    .updates
                    .iter()
                    .map(|update| {
                        FixedScalarUpdate::new(
                            update.ordinal,
                            update.expected.value(),
                            update.replacement.value(),
                        )
                    })
                    .collect();
                NativeFixedIntentGroup::new(name, requests, &mut budget(), &|| false).unwrap()
            })
            .collect();
        let mut native_work = ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000);
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            CatalogScope::new(118),
            &mut native_work,
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            image.plan_recoverable_fixed_updates("Items", groups, &mut native_work, &|| false),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::Column
            )))
        ));
    }
}

#[test]
fn fixed_metadata_uniqueness_is_paid_and_cancellable_without_database_rows() {
    let raw = literal();
    let mut measured = budget();
    let initial = [Resource::Bytes, Resource::Steps].map(|resource| measured.remaining(resource));
    let polls = Cell::new(0);
    NativeUpdateIntent::decode(&raw, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let costs = [
        initial[0] - measured.remaining(Resource::Bytes),
        initial[1] - measured.remaining(Resource::Steps),
    ];
    assert!(costs.iter().all(|cost| *cost > 0));
    let terminal = polls.get();
    for variant in 0..5 {
        let mut limits = costs;
        if variant < 2 {
            limits[variant] -= 1;
        }
        polls.set(0);
        let mut work = ResourceBudget::new(limits[0], 0, limits[1]);
        let result = NativeUpdateIntent::decode(&raw, &mut work, &|| {
            polls.set(polls.get() + 1);
            variant == 3 || (variant == 4 && polls.get() == terminal)
        });
        if variant == 2 {
            let decoded = result.unwrap();
            assert_eq!(decoded.fixed().unwrap().groups().len(), 6);
            assert_eq!(work.remaining(Resource::Rows), 0);
        } else if variant < 2 {
            let resource = [Resource::Bytes, Resource::Steps][variant];
            assert!(
                matches!(result, Err(NativeIntentError::Inspect(InspectError::Limit(error)))
                if error.resource == resource)
            );
        } else {
            assert!(matches!(
                result,
                Err(NativeIntentError::Inspect(InspectError::Cancelled))
            ));
        }
    }
}
