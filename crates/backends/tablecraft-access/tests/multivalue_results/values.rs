use super::*;

#[test]
fn bare_lists_preserve_parent_count_member_order_duplicates_null_and_empty() {
    for code in [2, 5, 6] {
        let original = custom_members(
            code,
            &[
                (10, Some(101), 1),
                (10, Some(100), 2),
                (10, Some(101), 3),
                (20, None, 4),
            ],
        );
        let source = NativeSource::new(
            Cursor::new(original.clone()),
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_multivalues();
        let rows = query(&source, "SELECT Id,Score FROM Items").unwrap();
        assert_eq!(
            observe(&rows),
            [
                (
                    1,
                    vec![Value::Int32(101), Value::Int32(100), Value::Int32(101)]
                ),
                (2, vec![Value::Null]),
                (3, vec![]),
            ]
        );
        let ordinary = parse_query("SELECT Id,Score.Value FROM Items", Default::default()).unwrap();
        let expanded = prepare(ordinary.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(
            expanded.rows(),
            [
                vec![Value::Int32(1), Value::Int32(101)],
                vec![Value::Int32(1), Value::Int32(100)],
                vec![Value::Int32(1), Value::Int32(101)],
                vec![Value::Int32(2), Value::Null],
                vec![Value::Int32(3), Value::Null],
            ]
        );
    }
}

#[test]
fn hidden_value_predicates_select_parents_without_truncating_the_returned_list() {
    for code in [2, 5, 6] {
        let source = NativeSource::new(
            Cursor::new(populated(code)),
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_multivalues();
        for sql in [
            "SELECT Id,Score FROM Items WHERE Score.Value=100",
            "SELECT Id,Score FROM Items WHERE Score.Value=100 AND Score.Value=101",
            "SELECT TOP 1 Id,Score FROM Items ORDER BY Id",
        ] {
            assert_eq!(
                observe(&query(&source, sql).unwrap()),
                [(1, vec![Value::Int32(100), Value::Int32(101)]),]
            );
        }
        assert_eq!(
            observe(&query(&source, "SELECT Id,Score FROM Items WHERE Id=3").unwrap()),
            [(3, vec![])],
        );
    }
}

#[test]
fn typed_lists_retain_single_ieee_bits_without_scalar_or_display_conversion() {
    use tablecraft_core::values::Single;

    const TEMPLATE_PAGE: usize = 17;
    const FIELD_OFFSET: usize = 63;
    const FIELD_BYTES: usize = 25;
    let bits = [0_u32, 0x8000_0000, 1, 0x7f80_0000, 0x7fc0_1234, 0xffc0_5678];
    let members = bits
        .iter()
        .enumerate()
        .map(|(position, bits)| (10, Some(*bits as i32), position as i32 + 1))
        .collect::<Vec<_>>();
    for code in [2, 5, 6] {
        let mut bytes = custom_members(code, &members);
        // Independent native Single tags retain the existing four-byte fixed geometry.
        bytes[TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET] = 6;
        bytes[CHILD_DEFINITION * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES] = 6;
        let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
            false
        })
        .unwrap()
        .with_multivalues();
        let rows = query(&source, "SELECT Id,Score FROM Items").unwrap();
        assert_eq!(
            observe(&rows),
            [
                (
                    1,
                    bits.map(|value| Value::Single(Single::from_bits(value)))
                        .to_vec()
                ),
                (2, vec![]),
                (3, vec![]),
            ],
        );
    }
}
