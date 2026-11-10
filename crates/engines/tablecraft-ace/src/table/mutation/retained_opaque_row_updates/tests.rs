//! Independent opaque prefix and full image witnesses, without retired-type inference.
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeEditPlan, NativeImage,
    check_table, inspect_table_schema, read_table,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::opaque_rows::{
    self as raw, CASES, Case,
};

const SCOPE: CatalogScope = CatalogScope::new(4318);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn value(value: Option<i32>) -> Value {
    value.map_or(Value::Null, Value::Int32)
}

fn plan(
    before: &[u8],
    row: usize,
    expected: Option<i32>,
    replacement: Option<i32>,
) -> Result<NativeEditPlan, EditError> {
    let requests = [FixedScalarUpdate::new(
        row,
        value(expected),
        value(replacement),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "NextNumber",
        updates: &requests,
    }];
    image(before).plan_retained_opaque_row_updates("Table1", &groups, &mut budget(), &|| false)
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

fn prefix_preserved(before: &[u8], after: &[u8], definition: u32, root: usize) {
    let old = raw::record(before, definition, root);
    let new = raw::record(after, definition, root);
    let count = usize::from(u16::from_le_bytes(old[..2].try_into().unwrap()));
    let mask = old.len() - count.div_ceil(8);
    for at in 2..mask {
        if !(6..10).contains(&at) {
            assert_eq!(old[at], new[at], "opaque prefix byte {at}");
        }
    }
}

#[test]
fn retained_opaque_rows_materialize_long_cells_and_transition_back_to_complete_rows() {
    for case in CASES {
        let source = raw::source(case);
        let native = image(&source.bytes);
        assert!(
            native
                .select_native_retained_opaque_rows_table("Table1", &mut budget(), &|| false)
                .unwrap()
        );
        native
            .check_native_retained_opaque_rows_table("Table1", &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            native.check_native_retained_rows_table("Table1", &mut budget(), &|| false),
            Err(EditError::Inspect(InspectError::Unsupported))
        ));
        let other = read_table(&mut native.reader(), SCOPE, "Other", &mut budget(), &|| {
            false
        })
        .unwrap();
        let mut before = source.bytes;
        let mut values = [None, None];
        for (row, replacement) in [
            (0, None),
            (0, Some(123)),
            (0, Some(-42)),
            (0, None),
            (1, None),
            (1, Some(789)),
        ] {
            let expected = raw::after(&before, source.definition, source.roots[row], replacement);
            let candidate = plan(&before, row, values[row], replacement)
                .unwrap_or_else(|error| panic!("{case:?}/row{row}/{replacement:?}: {error:?}"));
            equal_image(candidate.before_bytes(), &before);
            equal_image(candidate.after_bytes(), &expected);
            assert_eq!(
                candidate.changed_rows(),
                usize::from(values[row] != replacement)
            );
            prefix_preserved(
                &before,
                candidate.after_bytes(),
                source.definition,
                source.roots[row],
            );
            values[row] = replacement;
            let observed = read_table(
                &mut Cursor::new(candidate.after_bytes()),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                observed.rows(),
                &[
                    vec![Value::Int32(1), value(values[0])],
                    vec![Value::Int32(2), value(values[1])]
                ]
            );
            let sibling = read_table(
                &mut Cursor::new(candidate.after_bytes()),
                SCOPE,
                "Other",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(sibling.rows(), other.rows());
            assert_eq!(
                check_table(
                    &mut Cursor::new(candidate.after_bytes()),
                    SCOPE,
                    "Table1",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                2
            );
            before = expected;
        }
        let complete = image(&before);
        assert!(
            !complete
                .select_native_retained_opaque_rows_table("Table1", &mut budget(), &|| false)
                .unwrap()
        );
        complete
            .check_native_retained_rows_table("Table1", &mut budget(), &|| false)
            .unwrap();
        assert!(plan(&before, 0, None, Some(12)).is_err());
        let requests = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(12))];
        let groups = [FittedColumnUpdates::Fixed {
            column: "NextNumber",
            updates: &requests,
        }];
        let expected = raw::after(&before, source.definition, source.roots[0], Some(12));
        let old_family = complete
            .plan_retained_row_updates("Table1", &groups, &mut budget(), &|| false)
            .unwrap();
        equal_image(old_family.after_bytes(), &expected);
    }
}

#[test]
fn retained_opaque_rows_preserve_inactive_footer_bytes_and_reject_bad_fixed_bounds() {
    let source = raw::source(Case::VariableRetired);
    let mut before = source.bytes;
    let old = raw::record(&before, source.definition, 0);
    let count = usize::from(u16::from_le_bytes(old[..2].try_into().unwrap()));
    let mask = old.len() - count.div_ceil(8);
    assert!(mask > 10);
    let base = (source.definition as usize + 1) * 4096;
    let offset = old.as_ptr() as usize - before.as_ptr() as usize;
    assert_eq!(offset / 4096, base / 4096);
    for at in 10..mask {
        before[offset + at] ^= 0x5a;
    }
    let expected = raw::after(&before, source.definition, 0, Some(123));
    let candidate = plan(&before, 0, None, Some(123)).unwrap();
    equal_image(candidate.after_bytes(), &expected);
    prefix_preserved(&before, candidate.after_bytes(), source.definition, 0);

    for fixed in [0_u16, u16::MAX] {
        let mut malformed = before.to_vec();
        let at = source.definition as usize * 4096 + 100 + 21;
        malformed[at..at + 2].copy_from_slice(&fixed.to_le_bytes());
        assert!(plan(&malformed, 0, None, Some(123)).is_err());
    }
}

#[test]
fn retained_opaque_rows_keep_original_null_identity_purposes_and_control_failures() {
    let source = raw::source(Case::FixedRetired);
    // The retired Score bytes contain 17, but the new native number is absent.
    assert!(matches!(
        plan(&source.bytes, 0, Some(17), Some(123)),
        Err(EditError::Conflict)
    ));
    let native = image(&source.bytes);
    let schema = inspect_table_schema(
        &mut native.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let certificate =
        super::super::no_required_programs::NoRequiredTablePrograms::capture_retained_schema(
            &native,
            &schema,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(
        super::super::preserved_source::capture_retained_opaque_rows(
            &native,
            "Table1",
            &certificate,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    ));
    let requests = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(123))];
    let groups = [FittedColumnUpdates::Fixed {
        column: "NextNumber",
        updates: &requests,
    }];
    assert!(
        image(&source.bytes)
            .plan_retained_opaque_row_updates("Table1", &groups, &mut budget(), &|| true)
            .is_err()
    );
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(
            image(&source.bytes)
                .plan_retained_opaque_row_updates("Table1", &groups, &mut allowance, &|| false)
                .is_err()
        );
    }
}
