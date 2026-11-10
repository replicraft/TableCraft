use super::*;
use tablecraft_ace::inspect_table_schema;
use tablecraft_testkit::fixtures::access::mutation_fixture::historical_slots as raw;

#[test]
fn active_fields_above_ordinal_256_read_current_and_older_native_rows() {
    for profile in [1, 2, 3, 5, 6] {
        for variable in [false, true] {
            let bytes = raw::database(profile, variable);
            let table = read_table(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(table.schema().columns().len(), 2);
            assert_eq!(table.schema().columns()[1].number(), 300);
            assert_eq!(
                table.rows(),
                [
                    vec![
                        Value::Int32(1),
                        if variable {
                            Value::Text("AB".into())
                        } else {
                            Value::Int32(10)
                        }
                    ],
                    vec![Value::Int32(2), Value::Null],
                ]
            );
        }
    }
}

#[test]
fn historical_capacity_is_metadata_not_a_speculative_full_mask_allocation() {
    let mut bytes = raw::database(6, true);
    put16(&mut bytes, 10 * 4096 + 41, u16::MAX);
    put32(&mut bytes, 10 * 4096 + 16, 0);
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &[]));
    // Metadata inspection still pays for the three ordinary catalog records.
    let schema = inspect_table_schema(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        &mut ResourceBudget::new(131_072, 3, 100_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns().len(), 2);
    assert_eq!(schema.columns()[1].number(), 300);
}

#[test]
fn historical_slot_geometry_retains_corruption_budget_and_cancel_refusals() {
    for change in [0, 1, 2] {
        let mut bytes = raw::database(6, true);
        match change {
            0 => put16(&mut bytes, 10 * 4096 + 63 + 25 + 5, 301),
            1 => put16(&mut bytes, 10 * 4096 + 63 + 25 + 7, 300),
            _ => bytes[12 * 4096..13 * 4096]
                .copy_from_slice(&data_page(10, &[(0, 301_u16.to_le_bytes().to_vec())])),
        }
        assert!(
            read_table(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let bytes = raw::database(6, true);
    assert!(matches!(
        read_table(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut ResourceBudget::new(1024, 0, 100_000),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    assert!(matches!(
        read_table(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
}
