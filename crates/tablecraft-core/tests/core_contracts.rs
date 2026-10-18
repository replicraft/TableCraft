use tablecraft_core::ids::{
    CursorBookmark, CursorId, ObjectId, OperationId, RecordId, SourceGeneration, TableId,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_core::values::{
    Currency, Decimal, Double, ExpressionState, ExpressionValue, ExtendedDateTime, Guid,
    LegacyDateTime, Single, Value, ValueError,
};

#[test]
fn record_and_bookmark_identity_includes_its_owner_and_generation() {
    let table = TableId::from_u128(1);
    let generation = SourceGeneration::new(7);
    let record = RecordId::new(table, generation, vec![0, 255, 3]);
    assert_ne!(
        record,
        RecordId::new(TableId::from_u128(2), generation, vec![0, 255, 3])
    );
    assert_ne!(
        record,
        RecordId::new(table, SourceGeneration::new(8), vec![0, 255, 3])
    );
    assert_ne!(record, RecordId::new(table, generation, vec![0, 255, 4]));
    assert_eq!(record.table(), table);
    assert_eq!(record.generation(), generation);
    assert_eq!(record.token(), &[0, 255, 3]);

    let cursor = CursorId::from_u128(4);
    let bookmark = CursorBookmark::new(cursor, generation, vec![9]);
    assert_ne!(
        bookmark,
        CursorBookmark::new(CursorId::from_u128(5), generation, vec![9])
    );
    assert_ne!(
        bookmark,
        CursorBookmark::new(cursor, SourceGeneration::new(8), vec![9])
    );
    assert_eq!(bookmark.cursor(), cursor);
    assert_eq!(bookmark.generation(), generation);
    assert_eq!(bookmark.token(), &[9]);
}

#[test]
fn identity_tokens_survive_explicit_round_trips_without_numeric_truncation() {
    for token in [0, 1, u64::MAX as u128 + 1, u128::MAX] {
        assert_eq!(ObjectId::from_u128(token).as_u128(), token);
        assert_eq!(TableId::from_u128(token).as_u128(), token);
        assert_eq!(OperationId::from_u128(token).as_u128(), token);
        assert_eq!(CursorId::from_u128(token).as_u128(), token);
    }
    assert_eq!(SourceGeneration::new(u64::MAX).get(), u64::MAX);
}

#[test]
fn currency_preserves_entire_signed_range_and_rejects_overflow() {
    let min = Currency::from_scaled(i64::MIN);
    let max = Currency::from_scaled(i64::MAX);
    let unit = Currency::from_scaled(1);
    assert_eq!(min.scaled(), i64::MIN);
    assert_eq!(max.scaled(), i64::MAX);
    assert_eq!(min.to_string(), "-922337203685477.5808");
    assert_eq!(max.to_string(), "922337203685477.5807");
    assert_eq!(unit.to_string(), "0.0001");
    assert_eq!(Currency::from_scaled(-1).to_string(), "-0.0001");
    assert_eq!(max.checked_add(unit), Err(ValueError::CurrencyOverflow));
    assert_eq!(min.checked_sub(unit), Err(ValueError::CurrencyOverflow));
    assert_eq!(min.checked_add(max).unwrap().scaled(), -1);
    assert_eq!(max.checked_sub(max).unwrap().scaled(), 0);
}

#[test]
fn decimal_validates_access_envelope_without_panicking_on_i128_extremes() {
    let max = Decimal::MAX_COEFFICIENT;
    for scale in 0..=28 {
        for coefficient in [0, 1, -1, max, -max, 9_007_199_254_740_993] {
            let value = Decimal::new(coefficient, scale).unwrap();
            assert_eq!(value.coefficient(), coefficient);
            assert_eq!(value.scale(), scale);
            assert_eq!(
                Decimal::new(value.coefficient(), value.scale()).unwrap(),
                value
            );
        }
    }
    for coefficient in [max + 1, -max - 1, i128::MIN, i128::MAX] {
        assert_eq!(
            Decimal::new(coefficient, 0),
            Err(ValueError::DecimalOutOfRange)
        );
    }
    for scale in [29, u8::MAX] {
        assert_eq!(
            Decimal::new(0, scale),
            Err(ValueError::DecimalScaleOutOfRange)
        );
    }
}

#[test]
fn decimal_display_keeps_scale_and_digits_beyond_float_precision() {
    for (coefficient, scale, expected) in [
        (0, 4, "0.0000"),
        (1200, 3, "1.200"),
        (-1200, 3, "-1.200"),
        (9_007_199_254_740_993, 0, "9007199254740993"),
        (1, 28, "0.0000000000000000000000000001"),
        (-1, 28, "-0.0000000000000000000000000001"),
        (Decimal::MAX_COEFFICIENT, 0, "9999999999999999999999999999"),
    ] {
        assert_eq!(
            Decimal::new(coefficient, scale).unwrap().to_string(),
            expected
        );
    }
    // Representation equality deliberately preserves the supplied scale.
    assert_ne!(Decimal::new(10, 1).unwrap(), Decimal::new(100, 2).unwrap());
}

#[test]
fn decimal_rescaling_never_rounds_or_overflows_silently() {
    let value = Decimal::new(-1230, 3).unwrap();
    assert_eq!(
        value.rescale_exact(2).unwrap(),
        Decimal::new(-123, 2).unwrap()
    );
    assert_eq!(
        value.rescale_exact(4).unwrap(),
        Decimal::new(-12300, 4).unwrap()
    );
    assert_eq!(
        value.rescale_exact(1),
        Err(ValueError::DecimalPrecisionLoss)
    );
    assert_eq!(
        value.rescale_exact(29),
        Err(ValueError::DecimalScaleOutOfRange)
    );
    assert_eq!(
        Decimal::new(Decimal::MAX_COEFFICIENT, 0)
            .unwrap()
            .rescale_exact(1),
        Err(ValueError::DecimalOutOfRange)
    );
    assert_eq!(
        Decimal::new(Decimal::MAX_COEFFICIENT, 0)
            .unwrap()
            .rescale_exact(28),
        Err(ValueError::DecimalOutOfRange)
    );
    assert_eq!(
        Decimal::new(0, 0).unwrap().rescale_exact(28).unwrap(),
        Decimal::new(0, 28).unwrap()
    );
}

#[test]
fn deterministic_decimal_rescaling_sweep_round_trips_exact_values() {
    // Fixed, exhaustive small coefficients and every permitted scale: no RNG or locale.
    for coefficient in -100..=100 {
        for scale in 0..=28 {
            let original = Decimal::new(coefficient, scale).unwrap();
            for target in 0..=28 {
                if let Ok(rescaled) = original.rescale_exact(target) {
                    assert_eq!(rescaled.rescale_exact(scale).unwrap(), original);
                }
            }
        }
    }
}

#[test]
fn float_carriers_preserve_ieee_special_values_and_payloads() {
    for bits in [
        0,
        0x80000000,
        0x7f800000,
        0xff800000,
        0x7fc00001,
        0x7fc00002,
        u32::MAX,
    ] {
        assert_eq!(
            Single::from_float(Single::from_bits(bits).to_float()).to_bits(),
            bits
        );
    }
    for bits in [
        0,
        0x8000000000000000,
        0x7ff0000000000000,
        0xfff0000000000000,
        0x7ff8000000000001,
        0x7ff8000000000002,
        u64::MAX,
    ] {
        assert_eq!(
            Double::from_float(Double::from_bits(bits).to_float()).to_bits(),
            bits
        );
    }
    assert_ne!(Single::from_float(0.0), Single::from_float(-0.0));
    assert_eq!(
        Double::from_bits(0x7ff8000000000001),
        Double::from_bits(0x7ff8000000000001)
    );
}

#[test]
fn legacy_dates_keep_negative_date_rules_and_exact_bits() {
    for (days, day, fraction) in [
        (-2.5, -2, 0.5),
        (-0.25, 0, 0.25),
        (0.25, 0, 0.25),
        (2.5, 2, 0.5),
    ] {
        let value = LegacyDateTime::from_ole_days(days).unwrap();
        assert_eq!(value.day_offset(), day);
        assert_eq!(value.fraction_of_day(), fraction);
        assert_eq!(LegacyDateTime::from_bits(value.to_bits()).unwrap(), value);
        assert_eq!(value.ole_days().to_bits(), days.to_bits());
    }
    assert_ne!(
        LegacyDateTime::from_ole_days(-0.0).unwrap(),
        LegacyDateTime::from_ole_days(0.0).unwrap()
    );
    for days in [-657_434.999, -657_434.0, 2_958_465.999, 2_958_465.0] {
        assert!(LegacyDateTime::from_ole_days(days).is_ok());
    }
    for days in [
        -657_435.0,
        2_958_466.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert_eq!(
            LegacyDateTime::from_ole_days(days),
            Err(ValueError::InvalidLegacyDateTime)
        );
    }
}

#[test]
fn extended_dates_validate_calendar_and_full_fraction_without_timezone() {
    let min = ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap();
    let max = ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap();
    assert_eq!(min.date(), (1, 1, 1));
    assert_eq!(max.date(), (9999, 12, 31));
    assert_eq!(max.time(), (23, 59, 59, 9_999_999));
    for year in [4, 400, 1600, 2000, 2400] {
        assert!(ExtendedDateTime::new(year, 2, 29, 0, 0, 0, 0).is_ok());
    }
    for (year, month, day, hour, minute, second, fraction) in [
        (0, 1, 1, 0, 0, 0, 0),
        (10000, 1, 1, 0, 0, 0, 0),
        (1900, 2, 29, 0, 0, 0, 0),
        (2100, 2, 29, 0, 0, 0, 0),
        (2024, 0, 1, 0, 0, 0, 0),
        (2024, 13, 1, 0, 0, 0, 0),
        (2024, 4, 31, 0, 0, 0, 0),
        (2024, 1, 0, 0, 0, 0, 0),
        (2024, 1, 1, 24, 0, 0, 0),
        (2024, 1, 1, 0, 60, 0, 0),
        (2024, 1, 1, 0, 0, 60, 0),
        (2024, 1, 1, 0, 0, 0, 10_000_000),
    ] {
        assert_eq!(
            ExtendedDateTime::new(year, month, day, hour, minute, second, fraction),
            Err(ValueError::InvalidExtendedDateTime)
        );
    }
}

#[test]
fn storage_distinguishes_null_empty_text_width_and_expression_states() {
    assert_ne!(Value::Null, Value::Text(String::new()));
    assert_ne!(Value::Byte(1), Value::Int16(1));
    assert_ne!(Value::Int16(1), Value::Int32(1));
    assert_ne!(Value::Int32(1), Value::Int64(1));
    assert_eq!(
        Value::try_from(ExpressionValue::Value(Value::Null)),
        Ok(Value::Null)
    );
    assert_eq!(
        Value::try_from(ExpressionValue::Value(Value::Text(String::new()))),
        Ok(Value::Text(String::new()))
    );
    for (expression, state) in [
        (ExpressionValue::Empty, ExpressionState::Empty),
        (ExpressionValue::Missing, ExpressionState::Missing),
        (ExpressionValue::Error(42), ExpressionState::Error),
    ] {
        assert_eq!(
            Value::try_from(expression),
            Err(ValueError::ExpressionNotStorable(state))
        );
    }
    for number in [i64::MIN, 9_007_199_254_740_993, i64::MAX] {
        let value = Value::Int64(number);
        let Value::Int64(actual) = value else {
            panic!("wrong carrier");
        };
        assert_eq!(actual, number);
    }
}

#[test]
fn guid_bytes_stay_canonical_and_are_not_reinterpreted_as_native_layout() {
    let bytes = [
        0x6b, 0x29, 0xfc, 0x40, 0xca, 0x47, 0x10, 0x67, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
    ];
    assert_eq!(Guid::from_bytes(bytes).to_bytes(), bytes);
    assert_eq!(Guid::from_bytes([0; 16]).to_bytes(), [0; 16]);
}

#[test]
fn diagnostics_do_not_contain_cell_contents_or_locator_tokens() {
    assert_eq!(
        format!("{:?}", Value::Text("password=private".into())),
        "Text(\"[redacted]\")"
    );
    assert_eq!(
        format!("{:?}", Value::Binary(b"password=private".to_vec())),
        "Binary(\"[redacted]\")"
    );
    assert_eq!(
        format!("{:?}", Value::Int64(123456789)),
        "Int64(\"[redacted]\")"
    );
    assert_eq!(
        format!("{:?}", ExpressionValue::Error(123456789)),
        "Error([redacted])"
    );
    let record = RecordId::new(
        TableId::from_u128(1),
        SourceGeneration::new(2),
        b"password=private".to_vec(),
    );
    let bookmark = CursorBookmark::new(
        CursorId::from_u128(1),
        SourceGeneration::new(2),
        b"password=private".to_vec(),
    );
    for debug in [format!("{record:?}"), format!("{bookmark:?}")] {
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("password"));
        assert!(!debug.contains("112, 97"));
    }
    fn requires_error<T: std::error::Error>() {}
    requires_error::<ValueError>();
    requires_error::<LimitError>();
}

#[test]
fn budgets_enforce_independent_limits_and_leave_rejected_charges_unchanged() {
    let mut budget = ResourceBudget::new(10, 3, 4);
    budget.charge(Resource::Rows, 3).unwrap();
    assert_eq!(
        budget.charge(Resource::Rows, 1),
        Err(LimitError {
            resource: Resource::Rows,
            remaining: 0
        })
    );
    assert_eq!(budget.remaining(Resource::Rows), 0);
    assert_eq!(budget.remaining(Resource::Bytes), 10);
    assert_eq!(budget.remaining(Resource::Steps), 4);
    assert_eq!(
        budget.charge_allocation(3, 4),
        Err(LimitError {
            resource: Resource::Bytes,
            remaining: 10
        })
    );
    assert_eq!(budget.remaining(Resource::Bytes), 10);
    assert_eq!(budget.charge_allocation(5, 2).unwrap(), 10);
    budget.charge(Resource::Bytes, 0).unwrap();
    budget.charge(Resource::Steps, 4).unwrap();
    assert_eq!(budget.remaining(Resource::Bytes), 0);
    assert_eq!(budget.remaining(Resource::Steps), 0);
}

#[test]
fn counted_allocations_cannot_wrap_even_with_maximum_budget() {
    let mut budget = ResourceBudget::new(u64::MAX, 0, 0);
    assert_eq!(
        budget.charge_allocation(u64::MAX, 2),
        Err(LimitError {
            resource: Resource::Bytes,
            remaining: u64::MAX
        })
    );
    assert_eq!(budget.remaining(Resource::Bytes), u64::MAX);
    assert_eq!(budget.charge_allocation(u64::MAX, 0).unwrap(), 0);
    assert_eq!(budget.charge_allocation(u64::MAX, 1).unwrap(), u64::MAX);
    assert!(budget.charge_allocation(1, 1).is_err());
}
