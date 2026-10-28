//! Row-capacity choices are complete before the external allocation phase.
use super::*;
use crate::table::mutation::{
    insert::record::input::{self, Payload},
    long_value::encode::{Encoded, StorageChoice},
};

fn chosen(payload: &Payload, expected: &[u8]) -> StorageChoice {
    let Payload::Long {
        encoded: Encoded::Present(bytes),
        choice,
    } = payload
    else {
        panic!("expected a present long payload");
    };
    assert_eq!(bytes, expected);
    *choice
}

#[test]
fn long_value_row_pressure_uses_variable_order_before_any_reservation() {
    for code in [5, 6] {
        let bytes = ordinary(
            code,
            &[
                ("ID", 4, 3970, 4),
                ("First", 11, 0, 0),
                ("Second", 11, 0, 0),
            ],
            &[(1, 1, false), (2, 0, false)],
        );
        let metadata = schema(&bytes);
        let profile = Profile::bind(&metadata, &mut budget(), &|| false).unwrap();
        assert_eq!(profile.variables(), &[2, 1]);
        let values = [
            Value::Int32(1),
            Value::Binary(vec![0x31; 64]),
            Value::Binary(vec![0x52; 64]),
        ];
        let input = input::prepare(
            &metadata,
            profile.variables(),
            &values,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            chosen(&input.payloads[0], &[0x52; 64]),
            StorageChoice::Single
        ));
        assert!(matches!(
            chosen(&input.payloads[1], &[0x31; 64]),
            StorageChoice::Inline
        ));
    }
}

#[test]
fn empty_and_null_long_values_never_become_external_capacity_placeholders() {
    for code in [5, 6] {
        let bytes = ordinary(
            code,
            &[("ID", 4, 4024, 4), ("Empty", 11, 0, 0), ("Other", 11, 0, 0)],
            &[(1, 0, false), (2, 1, false)],
        );
        let metadata = schema(&bytes);
        let profile = Profile::bind(&metadata, &mut budget(), &|| false).unwrap();
        let input = input::prepare(
            &metadata,
            profile.variables(),
            &[
                Value::Int32(1),
                Value::Binary(Vec::new()),
                Value::Binary(vec![0x73; 64]),
            ],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            chosen(&input.payloads[0], &[]),
            StorageChoice::Inline
        ));
        assert!(matches!(
            &input.payloads[1],
            Payload::Long {
                choice: StorageChoice::Single,
                ..
            }
        ));

        let bytes = ordinary(
            code,
            &[
                ("ID", 4, 4054, 4),
                ("Empty", 11, 0, 0),
                ("Absent", 11, 0, 0),
            ],
            &[(1, 0, false), (2, 1, false)],
        );
        let metadata = schema(&bytes);
        let profile = Profile::bind(&metadata, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            input::prepare(
                &metadata,
                profile.variables(),
                &[Value::Int32(1), Value::Binary(Vec::new()), Value::Null],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
    }
}

#[test]
fn long_value_preflight_retains_codec_choices_and_checks_all_fixed_carriers() {
    let bytes = ordinary(
        6,
        &[("ID", 4, 0, 4), ("Raw", 11, 0, 0), ("Text", 12, 0, 0)],
        &[(1, 0, false), (2, 1, true)],
    );
    let metadata = schema(&bytes);
    let profile = Profile::bind(&metadata, &mut budget(), &|| false).unwrap();
    let values = [
        Value::Int32(1),
        Value::Binary(vec![0x81; 4077]),
        Value::Text("ABC".into()),
    ];
    let input = input::prepare(
        &metadata,
        profile.variables(),
        &values,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        chosen(&input.payloads[0], &vec![0x81; 4077]),
        StorageChoice::Chained
    ));
    assert!(matches!(
        chosen(&input.payloads[1], &[255, 254, 65, 66, 67]),
        StorageChoice::Inline
    ));
    let bad = [
        Value::Currency(Currency::from_scaled(1)),
        Value::Binary(vec![0x81; 4077]),
        Value::Text("ABC".into()),
    ];
    assert!(matches!(
        input::prepare(&metadata, profile.variables(), &bad, &mut budget(), &|| {
            false
        }),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}

#[test]
fn long_value_preflight_pays_for_payloads_and_observes_terminal_cancellation() {
    let bytes = ordinary(6, &[("ID", 4, 0, 4), ("Raw", 11, 0, 0)], &[(1, 0, false)]);
    let metadata = schema(&bytes);
    let profile = Profile::bind(&metadata, &mut budget(), &|| false).unwrap();
    let values = [Value::Int32(1), Value::Binary(vec![0xa5; 4077])];
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 1_000_000),
        ResourceBudget::new(8 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            input::prepare(
                &metadata,
                profile.variables(),
                &values,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let healthy = Cell::new(0);
    input::prepare(
        &metadata,
        profile.variables(),
        &values,
        &mut budget(),
        &|| {
            healthy.set(healthy.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(healthy.get() > 1);
    let visits = Cell::new(0);
    assert!(matches!(
        input::prepare(
            &metadata,
            profile.variables(),
            &values,
            &mut budget(),
            &|| {
                visits.set(visits.get() + 1);
                visits.get() == healthy.get()
            }
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(visits.get(), healthy.get());
}
