use super::{checked, corpus, numeric_fixture};

#[test]
fn every_existing_fixed_numeric_profile_checks_its_actual_non_null_population() {
    // Each case is an independent stored-bit golden, never production key output.
    let cases: &[(u8, &[Option<i64>])] = &[
        (1, &[Some(0), Some(-1), Some(0), Some(-1)]),
        (2, &[None, Some(0), Some(128), Some(255), None]),
        (3, &[None, Some(-32768), Some(-1), Some(0), Some(32767)]),
        (4, &[None, Some(-2147483648), Some(0), Some(2147483647)]),
        (5, &[None, Some(i64::MIN), Some(0), Some(i64::MAX)]),
        (
            6,
            &[
                None,
                Some(0x80000000),
                Some(0),
                Some(0x7f800000),
                Some(0x7fc00001),
            ],
        ),
        (
            7,
            &[
                None,
                Some(i64::MIN),
                Some(0),
                Some(0x7ff0000000000000),
                Some(0x7ff8000000000001),
            ],
        ),
        (
            8,
            &[
                None,
                Some(0xbff0000000000000_u64 as i64),
                Some(0),
                Some(0x3ff0000000000000),
            ],
        ),
        (
            0x13,
            &[None, Some(i64::MIN), Some(9007199254740993), Some(i64::MAX)],
        ),
    ];
    for code in [1, 2, 5, 6] {
        for &(kind, values) in cases {
            if kind == 0x13 && !matches!(code, 5 | 6) {
                continue;
            }
            for descending in [false, true] {
                let rows: Vec<_> = values
                    .iter()
                    .enumerate()
                    .map(|(slot, value)| vec![Some(slot as i64 + 1), *value])
                    .collect();
                let omitted: Vec<_> = values
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, value)| value.is_none().then_some(slot))
                    .collect();
                let mut bytes =
                    numeric_fixture::database(code, &[4, kind], &[(1, descending)], &rows, 0);
                if code == 1 {
                    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
                }
                let bytes = corpus::ignoring(bytes, &omitted, false);
                let original = bytes.clone();
                checked(&bytes, rows.len());
                assert_eq!(bytes, original);
            }
        }
    }
}
