//! Immutable capture/source certification and retained non-fixed policy refusals.
mod api;
use support as raw;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as base;

#[allow(dead_code)]
#[path = "../../../../../tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../../../tests/ace16_larger_fixed_images/corrected.rs"]
mod corrected;
#[path = "../../../../../tests/ace16_larger_fixed_images/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../../../../../tests/attachment_modern/fixture.rs"]
mod modern_attachment;
#[allow(dead_code)]
#[path = "../../../../../tests/attachments/fixture.rs"]
mod old_attachment;
#[path = "../../../../../tests/ace16_preserved_scalar_updates_contract/model.rs"]
mod preserved_model;

use crate::{
    BeforeEventGuardError, BeforeEventIntents, EditError, FixedUpdateImageLimits,
    IncrementingAutoNumber, InspectError, Int32Update, NativeImage, RowDelete, SlotScalarUpdate,
    WriteBlocker, inspect_table_schema,
    table::mutation::{gates, variable_split::metadata::Metadata},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(1);
const WORK_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 1_000_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(WORK_BYTES, WORK_ROWS, WORK_STEPS)
}

fn capture(bytes: &[u8], ceiling: u64) -> Result<NativeImage, InspectError> {
    NativeImage::capture_for_fixed_updates(
        &mut Cursor::new(bytes),
        SCOPE,
        FixedUpdateImageLimits::new(ceiling)?,
        &mut budget(),
        &|| false,
    )
}

fn small_image(code: u8) -> NativeImage {
    let mut bytes = base::database();
    if code != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes[20] = code;
    capture(&bytes, model::EXPLICIT_BYTES as u64).unwrap()
}

struct ForbiddenClock;
impl Clock for ForbiddenClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        panic!("capture-only policy must refuse before clock evaluation")
    }
}

#[test]
fn validated_fixed_capture_limits_leave_default_and_budgets_independent() {
    assert!(matches!(
        FixedUpdateImageLimits::new(0),
        Err(InspectError::StructureLimit)
    ));
    assert!(matches!(
        FixedUpdateImageLimits::new(FixedUpdateImageLimits::MAX_BYTES + 1),
        Err(InspectError::StructureLimit)
    ));
    assert_eq!(
        FixedUpdateImageLimits::default().max_bytes(),
        64 * 1024 * 1024
    );
    let bytes = base::database();
    assert_eq!(
        capture(&bytes, bytes.len() as u64).unwrap().as_bytes(),
        bytes
    );
    assert!(matches!(
        capture(&bytes, bytes.len() as u64 - 1),
        Err(InspectError::StructureLimit)
    ));

    let mut measured = budget();
    let polls = Cell::new(0);
    NativeImage::capture_for_fixed_updates(
        &mut Cursor::new(&bytes),
        SCOPE,
        FixedUpdateImageLimits::default(),
        &mut measured,
        &|| {
            polls.set(polls.get() + 1);
            false
        },
    )
    .unwrap();
    for (resource, available) in [
        (Resource::Bytes, WORK_BYTES),
        (Resource::Rows, WORK_ROWS),
        (Resource::Steps, WORK_STEPS),
    ] {
        let needed = available - measured.remaining(resource);
        assert!(needed > 0);
        for (quota, success) in [(needed, true), (needed - 1, false)] {
            let (bytes_limit, rows_limit, steps_limit) = match resource {
                Resource::Bytes => (quota, WORK_ROWS, WORK_STEPS),
                Resource::Rows => (WORK_BYTES, quota, WORK_STEPS),
                Resource::Steps => (WORK_BYTES, WORK_ROWS, quota),
            };
            let mut limits = ResourceBudget::new(bytes_limit, rows_limit, steps_limit);
            let result = NativeImage::capture_for_fixed_updates(
                &mut Cursor::new(&bytes),
                SCOPE,
                FixedUpdateImageLimits::default(),
                &mut limits,
                &|| false,
            );
            if success {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(InspectError::Limit(_))));
            }
        }
    }
    let stop = polls.get();
    let polls = Cell::new(0);
    assert!(matches!(
        NativeImage::capture_for_fixed_updates(
            &mut Cursor::new(&bytes),
            SCOPE,
            FixedUpdateImageLimits::default(),
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            }
        ),
        Err(InspectError::Cancelled)
    ));
}

#[test]
fn qualify_actual_extended_capture_and_ownership_for_prefrozen_large_sources() {
    let mut images = 0;
    let mut admitted = 0;
    let mut refused = 0;
    for code in [5, 6] {
        for shape in model::Shape::ALL {
            let case = corrected::case(code, shape);
            for bytes in [&case.before, &case.expected] {
                let image = capture(bytes, model::EXPLICIT_BYTES as u64).unwrap();
                assert_eq!(image.as_bytes(), bytes);
                assert!(matches!(
                    image.require_standard_capture(),
                    Err(InspectError::StructureLimit)
                ));
                if shape.preserved() && code == 5 {
                    let schema = inspect_table_schema(
                        &mut image.reader(),
                        SCOPE,
                        "Items",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    for (number, name) in [(7, "Date_Extended_Field"), (8, "Time_Extended_Field")] {
                        let field = &schema.columns()[number];
                        assert_eq!(field.number(), number);
                        assert_eq!(field.name(), name);
                        assert_eq!(field.native_type(), 20);
                        assert_eq!(field.storage_length(), 42);
                        assert!(field.fixed());
                        assert_eq!(field.extended_flags(), 0);
                        assert_eq!(field.scalar_type(), None);
                    }
                    assert!(matches!(
                        super::source::capture(&image, "Items", &mut budget(), &|| false),
                        Err(EditError::Unsupported(WriteBlocker::Column))
                    ));
                    refused += 1;
                } else if shape.preserved() {
                    let source =
                        super::source::capture(&image, "Items", &mut budget(), &|| false).unwrap();
                    assert_eq!(source.records.len(), 3);
                    for (slot, record) in source.records.iter().enumerate() {
                        assert_eq!(record.reference, ((case.data as u32) << 8) | slot as u32);
                        assert_eq!(record.offset / model::PAGE, case.data);
                    }
                    admitted += 1;
                } else {
                    let checked =
                        gates::before_fixed_update(&image, "Items", &mut budget(), &|| false)
                            .unwrap();
                    let metadata = Metadata::capture_for_fitted_updates(
                        &image,
                        checked.schema(),
                        checked.indexes(),
                        &[1],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    metadata
                        .require_source(&image, checked.schema().definition_page())
                        .unwrap();
                    metadata
                        .require_data_page(case.data as u32, &mut budget(), &|| false)
                        .unwrap();
                    admitted += 1;
                }
                images += 1;
            }
        }
        assert!(matches!(
            capture(&model::over_cap_source(code), model::EXPLICIT_BYTES as u64),
            Err(InspectError::StructureLimit)
        ));
    }
    assert_eq!(images, 24);
    assert_eq!(admitted, 20);
    assert_eq!(refused, 4);
}

#[test]
fn all_other_public_mutation_entries_refuse_extended_captures_including_empty_intents() {
    macro_rules! denied {
        ($result:expr) => {
            assert!(matches!(
                $result,
                Err(EditError::Inspect(InspectError::StructureLimit))
            ));
        };
    }
    for code in [1, 5, 6] {
        let policy = IncrementingAutoNumber::new(3);
        let clock = ForbiddenClock;
        denied!(small_image(code).plan_int32_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_int32_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_page_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_variable_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_split_int32_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_split_fixed_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_split_slot_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_variable_split_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_long_value_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_fitted_updates(
            "Items",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_mixed_updates(
            "Items",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_mixed_split_updates(
            "Items",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_deletes("Items", &[], &mut budget(), &|| false));
        denied!(small_image(code).preview_ace16_batch_deletes(&[], &mut budget(), &|| false));
        denied!(small_image(code).preview_ace16_cascading_deletes(&[], &mut budget(), &|| false));
        denied!(
            small_image(code).prepare_ace16_row_deletes("Items", &[], &mut budget(), &|| false)
        );
        denied!(
            small_image(code).preview_ace16_insert_rows("Items", &[], &mut budget(), &|| false)
        );
        denied!(small_image(code).preview_ace16_incrementing_insert_rows(
            "Items",
            &[],
            policy,
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).prepare_ace16_incrementing_insert_policy(
            "Items",
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).prepare_ace16_incrementing_insert_binding(
            "Items",
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_insert_rows_with_omissions(
            "Items",
            &[],
            &mut budget(),
            &|| false
        ));
        denied!(
            small_image(code).preview_ace16_incrementing_insert_rows_with_omissions(
                "Items",
                &[],
                policy,
                &mut budget(),
                &|| false
            )
        );
        denied!(
            small_image(code).preview_ace16_insert_rows_with_omissions_and_clock(
                "Items",
                &[],
                &clock,
                &mut budget(),
                &|| false
            )
        );
        denied!(
            small_image(code).preview_ace16_incrementing_insert_rows_with_omissions_and_clock(
                "Items",
                &[],
                policy,
                &clock,
                &mut budget(),
                &|| false
            )
        );
        assert!(matches!(
            small_image(code).prepare_ace16_before_event_guards(
                "Items",
                &[],
                BeforeEventIntents::Insert(&[]),
                &mut budget(),
                &|| false
            ),
            Err(BeforeEventGuardError::Inspect(InspectError::StructureLimit))
        ));

        denied!(small_image(code).preview_ace16_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(0, 10, 10)],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_slot_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(0, Value::Int32(10), Value::Int32(10))],
            &mut budget(),
            &|| false
        ));
        denied!(small_image(code).preview_ace16_deletes(
            "Items",
            &[RowDelete::new(0, vec![])],
            &mut budget(),
            &|| false
        ));
    }
}
