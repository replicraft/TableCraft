//! Genuine metadata ADD preserves short rows until a Boolean is materialized.
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeEditPlan, NativeImage,
    WriteBlocker, read_table,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{fields, fixed_scalars, typed_fields},
    raw_forwarding_chain,
};

const PAGE_BYTES: usize = 4096;
const CATALOG_PAGE: usize = 4;
const ROW_COUNT_OFFSET: usize = 12;
const MASK_BITS: usize = 8;
const ORIGINAL_MASK: usize = 10;
const SCOPE: CatalogScope = CatalogScope::new(4323);

#[derive(Clone, Copy)]
enum Shape {
    Plain,
    WiderMask,
}

struct Source {
    bytes: Vec<u8>,
    definition: u32,
    catalog_root: usize,
    table: &'static str,
    number: usize,
    fixed_stop: usize,
    selected_row: usize,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn source(shape: Shape, garbage: bool) -> Source {
    let seed = fields::source(fields::Case::FirstNumber);
    let mut source = match shape {
        Shape::Plain => Source {
            bytes: seed.bytes,
            definition: seed.definition,
            catalog_root: seed.catalog_root,
            table: "Table1",
            number: 2,
            fixed_stop: 10,
            selected_row: 0,
        },
        Shape::WiderMask => {
            let definition = (seed.bytes.len() / PAGE_BYTES) as u32;
            let count_at = CATALOG_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET;
            let catalog_root = usize::from(u16::from_le_bytes(
                seed.bytes[count_at..count_at + size_of::<u16>()]
                    .try_into()
                    .unwrap(),
            ));
            let empty = fixed_scalars::append_table(&seed.bytes, "BooleanValues");
            let first = fixed_scalars::historical::append(&empty, definition, 124);
            let before = fixed_scalars::historical::append(&first, definition, -73);
            Source {
                bytes: before,
                definition,
                catalog_root,
                table: "BooleanValues",
                number: 8,
                fixed_stop: 41,
                selected_row: 1,
            }
        }
    };
    source.bytes = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Enabled",
        typed_fields::Kind::Boolean,
    );
    if garbage {
        for row in 0..2 {
            let mut record =
                raw_forwarding_chain::record(&source.bytes, source.definition + 1, row).to_vec();
            assert_eq!(record.len(), ORIGINAL_MASK + 1);
            record[ORIGINAL_MASK] |= 0xfc;
            source.bytes = raw_forwarding_chain::replace_terminal(
                &source.bytes,
                source.definition + 1,
                row,
                &record,
            );
        }
    }
    source
}

fn expected(source: &Source) -> Vec<u8> {
    let original =
        raw_forwarding_chain::record(&source.bytes, source.definition + 1, source.selected_row);
    assert_eq!(original.len(), ORIGINAL_MASK + 1);
    assert_eq!(&original[..2], &[2, 0]);

    let fields = source.number + 1;
    let mut record = vec![0; source.fixed_stop + fields.div_ceil(MASK_BITS)];
    record[..2].copy_from_slice(&(fields as u16).to_le_bytes());
    record[2..ORIGINAL_MASK].copy_from_slice(&original[2..ORIGINAL_MASK]);
    record[source.fixed_stop] = original[ORIGINAL_MASK] & 3;
    if fields <= MASK_BITS {
        // Inert padding above the new count belongs to the original byte history.
        let represented = ((1_u16 << fields) - 1) as u8;
        record[source.fixed_stop] |= original[ORIGINAL_MASK] & !represented;
    }
    record[source.fixed_stop + source.number / MASK_BITS] |= 1 << (source.number % MASK_BITS);
    raw_forwarding_chain::place_terminal(
        &source.bytes,
        source.definition + 1,
        source.selected_row,
        &record,
    )
}

fn plan(
    before: &[u8],
    table: &str,
    row: usize,
    old: bool,
    value: bool,
) -> Result<NativeEditPlan, EditError> {
    let requests = [FixedScalarUpdate::new(
        row,
        Value::Boolean(old),
        Value::Boolean(value),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Enabled",
        updates: &requests,
    }];
    image(before).plan_retained_boolean_materialization(table, &groups, &mut budget(), &|| false)
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None
    );
}

#[test]
fn retained_boolean_materialization_preserves_short_noops_and_exact_expanded_masks() {
    for shape in [Shape::Plain, Shape::WiderMask] {
        for garbage in [false, true] {
            let source = source(shape, garbage);
            let values = read_table(
                &mut Cursor::new(&source.bytes),
                SCOPE,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                values
                    .rows()
                    .iter()
                    .all(|row| row[source.number] == Value::Boolean(false))
            );
            image(&source.bytes)
                .check_native_retained_boolean_materialization_table(
                    source.table,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();

            let noop = plan(
                &source.bytes,
                source.table,
                source.selected_row,
                false,
                false,
            )
            .unwrap();
            assert_eq!(noop.changed_rows(), 0);
            assert_image(noop.after_bytes(), &source.bytes);

            let after = expected(&source);
            let changed = plan(
                &source.bytes,
                source.table,
                source.selected_row,
                false,
                true,
            )
            .unwrap();
            assert_eq!(changed.changed_rows(), 1);
            assert_image(changed.after_bytes(), &after);

            let values = read_table(
                &mut Cursor::new(&after),
                SCOPE,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                values.rows()[source.selected_row][source.number],
                Value::Boolean(true)
            );
            assert_eq!(
                values.rows()[1 - source.selected_row][source.number],
                Value::Boolean(false)
            );
            if matches!(shape, Shape::WiderMask) {
                assert!(
                    values.rows()[source.selected_row][2..source.number]
                        .iter()
                        .all(|value| *value == Value::Null)
                );
            }
            assert!(matches!(
                plan(&after, source.table, source.selected_row, true, false),
                Err(EditError::Unsupported(WriteBlocker::RowLayout)),
            ));

            // The existing action now owns the stored bit, even while another
            // source row still has the historical shorter count.
            let requests = [FixedScalarUpdate::new(
                source.selected_row,
                Value::Boolean(true),
                Value::Boolean(false),
            )];
            let groups = [FittedColumnUpdates::Fixed {
                column: "Enabled",
                updates: &requests,
            }];
            let stored = image(&after)
                .plan_retained_boolean_row_updates(source.table, &groups, &mut budget(), &|| false)
                .unwrap();
            let mut record =
                raw_forwarding_chain::record(&after, source.definition + 1, source.selected_row)
                    .to_vec();
            record[source.fixed_stop + source.number / MASK_BITS] &=
                !(1 << (source.number % MASK_BITS));
            let expected = raw_forwarding_chain::replace_terminal(
                &after,
                source.definition + 1,
                source.selected_row,
                &record,
            );
            assert_image(stored.after_bytes(), &expected);
        }
    }
}

#[test]
fn retained_boolean_materialization_rechecks_expected_value_carriers_and_resources() {
    let source = source(Shape::Plain, true);
    assert!(matches!(
        plan(&source.bytes, source.table, 0, true, true),
        Err(EditError::Conflict),
    ));

    let requests = [FixedScalarUpdate::new(
        0,
        Value::Boolean(false),
        Value::Null,
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Enabled",
        updates: &requests,
    }];
    assert!(matches!(
        image(&source.bytes).plan_retained_boolean_materialization(
            source.table,
            &groups,
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::Column)),
    ));

    let requests = [FixedScalarUpdate::new(
        0,
        Value::Boolean(false),
        Value::Boolean(true),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Enabled",
        updates: &requests,
    }];
    assert!(
        image(&source.bytes)
            .plan_retained_boolean_row_updates(source.table, &groups, &mut budget(), &|| false,)
            .is_err()
    );
    assert!(matches!(
        image(&source.bytes).plan_retained_boolean_materialization(
            source.table,
            &groups,
            &mut budget(),
            &|| true,
        ),
        Err(EditError::Inspect(InspectError::Cancelled)),
    ));
    for mut work in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(matches!(
            image(&source.bytes).plan_retained_boolean_materialization(
                source.table,
                &groups,
                &mut work,
                &|| false,
            ),
            Err(EditError::Inspect(InspectError::Limit(_))),
        ));
    }
}
