use super::*;
use tablecraft_ace::{WriteBlocker, inspect_table_schema};

const OTHER_DEFINITION: usize = 15;
const OTHER_MAP: usize = 16;
const OTHER_DATA: usize = 17;
const OTHER_LONG_VALUE: usize = 18;
const OTHER_FIELD: usize = 88;
const FIELD_FLAGS: usize = 15;
const FIELD_EXTENDED_FLAGS: usize = 16;
const SLOT_COUNT: usize = 12;
const DELETED_RELOCATED: u16 = 0xc000;

fn backing_source(code: u8, aliased: bool) -> Vec<u8> {
    let mut bytes = source(code, 1);
    other_table_alias(&mut bytes, true, true);
    let field = OTHER_DEFINITION * PAGE + OTHER_FIELD;
    bytes[field] = 11;
    bytes[field + FIELD_FLAGS] = 2;
    bytes[field + FIELD_EXTENDED_FLAGS] = 0x10;

    let owned = if aliased { MEMO } else { OTHER_LONG_VALUE };
    install(
        &mut bytes,
        OTHER_MAP,
        &page(
            0,
            &[
                (0, map(&[OTHER_DATA])),
                (0, map(&[OTHER_DATA])),
                (0, map(&[owned])),
                (0, map(&[owned])),
            ],
        ),
    );
    install(
        &mut bytes,
        OTHER_LONG_VALUE,
        &lval(&[(0, vec![0x91, 0x72])]),
    );
    let descriptor = single(2, owned, 0);
    let mut row = vec![2, 0, 1, 0, 0, 0];
    row.extend(descriptor);
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(6_u16.to_le_bytes());
    row.extend(1_u16.to_le_bytes());
    row.push(3);
    install(&mut bytes, OTHER_DATA, &page(15, &[(0, row)]));
    bytes
}

#[test]
fn backing_ole_is_censused_without_granting_selected_write_authority() {
    for code in [5, 6] {
        for aliased in [false, true] {
            let bytes = backing_source(code, aliased);
            let schema = inspect_table_schema(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Other",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let field = &schema.columns()[1];
            assert_eq!(field.native_type(), 11);
            assert_eq!(field.flags(), 2);
            assert_eq!(field.extended_flags(), 0x10);
            assert!(!field.fixed());
            assert!(field.scalar_type().is_none());
            let report = check_table_long_values(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Other",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                report.columns()[0].status(),
                LongValueCheckStatus::UnsupportedColumn
            );
            check(&bytes);

            let result = run(&bytes, "Memo", &[memo_update(Value::Null)]);
            if aliased {
                assert_eq!(
                    result.err(),
                    Some(EditError::Unsupported(WriteBlocker::RowLayout))
                );
            } else {
                let candidate = result.unwrap();
                assert_eq!(read(candidate.candidate_bytes())[0][1], Value::Null);
                for number in [OTHER_DEFINITION, OTHER_MAP, OTHER_DATA, OTHER_LONG_VALUE] {
                    assert_eq!(
                        &candidate.candidate_bytes()[number * PAGE..(number + 1) * PAGE],
                        &bytes[number * PAGE..(number + 1) * PAGE]
                    );
                }
            }
        }
    }
}

#[test]
fn deleted_relocated_empty_slots_are_skipped_but_live_empty_slots_are_malformed() {
    for code in [5, 6] {
        let mut bytes = source(code, 1);
        other_table_alias(&mut bytes, false, false);
        let start = word(&bytes, OTHER_DATA, 0) & OFFSET_MASK;
        support::put16(&mut bytes, OTHER_DATA * PAGE + SLOT_COUNT, 2);
        support::put16(
            &mut bytes,
            OTHER_DATA * PAGE + SLOT + 2,
            DELETED_RELOCATED | start,
        );
        assert_eq!(word(&bytes, OTHER_DATA, 1), DELETED_RELOCATED | start);
        assert_eq!(word(&bytes, OTHER_DATA, 1) & OFFSET_MASK, start);
        check(&bytes);
        let checked = check_table_long_values(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 1);
        let result = run(&bytes, "Memo", &[memo_update(Value::Null)]).unwrap();
        assert_eq!(read(result.candidate_bytes())[0][1], Value::Null);
        assert_eq!(
            &result.candidate_bytes()[OTHER_DATA * PAGE..(OTHER_DATA + 1) * PAGE],
            &bytes[OTHER_DATA * PAGE..(OTHER_DATA + 1) * PAGE]
        );

        support::put16(&mut bytes, OTHER_DATA * PAGE + SLOT + 2, start);
        assert!(matches!(
            check_table_long_values(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Other",
                &mut budget(),
                &|| false,
            ),
            Err(InspectError::Malformed)
        ));
        assert_eq!(
            run(&bytes, "Memo", &[]).err(),
            Some(EditError::Inspect(InspectError::Malformed))
        );
    }
}
