//! Source-owned LVAL pages, literal chains and complete expected images.
use super::{Family, PAGE, checked, frames, original, payload, raw, source};
use crate::{EditError, InspectError, LongValueCheckStatus, RowInsert, check_table_long_values};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const LVAL: u32 = 16;
const EOF: u32 = 18;

fn lval_page(body: Option<Vec<u8>>) -> Vec<u8> {
    let records = body.into_iter().map(|body| (0, body)).collect::<Vec<_>>();
    let length = records.iter().map(|(_, body)| body.len()).sum::<usize>();
    let mut page = raw::data_page(0, &records);
    page[4..8].copy_from_slice(b"LVAL");
    raw::put16(
        &mut page,
        2,
        (PAGE - 14 - 2 * records.len() - length) as u16,
    );
    page
}

fn column_maps(bytes: &mut [u8], owned: &[u32], hints: &[u32]) {
    frames::map_page(
        bytes,
        11,
        vec![
            frames::map([12]),
            frames::map([12]),
            frames::map(owned.iter().copied()),
            frames::map(hints.iter().copied()),
        ],
    );
}

pub(in crate::table::mutation::insert) fn original(code: u8, family: Family) -> frames::Case {
    let empty = if matches!(family, Family::Ole) {
        Value::Binary(vec![])
    } else {
        Value::Text(String::new())
    };
    let small = if matches!(family, Family::Ole) {
        Value::Binary(vec![0, 128, 255])
    } else {
        Value::Text("ABC".into())
    };
    let mut case = source(code, family, &[Value::Null, empty, small]);
    case.before.resize(EOF as usize * PAGE, 0);
    // Actual column-map declarations follow the four logical index names.
    let declaration = 10 * PAGE + 498;
    assert_eq!(&case.before[declaration..declaration + 2], &[255, 255]);
    raw::put16(&mut case.before, declaration, 2);
    raw::put32(&mut case.before, declaration + 2, (11 << 8) | 2);
    raw::put32(&mut case.before, declaration + 6, (11 << 8) | 3);
    raw::put16(&mut case.before, declaration + 10, 0xffff);
    // Explicit complete empty declarations for system-column LVAL inventories.
    raw::put16(&mut case.before, 2 * PAGE + 398, 0xffff);
    raw::put16(&mut case.before, 7 * PAGE + 449, 0xffff);
    column_maps(&mut case.before, &[LVAL], &[LVAL]);
    frames::install(&mut case.before, LVAL, &lval_page(None));
    frames::map_page(&mut case.before, 1, vec![frames::map(EOF..512)]);
    case
}

fn check_lval(bytes: &[u8]) {
    let report = check_table_long_values(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.columns().len(), 1);
    assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Checked);
    assert_eq!(report.columns()[0].shared_references(), 0);
    assert_eq!(report.columns()[0].shared_owned_pages(), 0);
}

pub(in crate::table::mutation::insert) fn expected(
    case: &frames::Case,
    family: Family,
    value: &Value,
) -> Vec<u8> {
    let payload = payload(value, family).unwrap();
    assert!(payload.len() > 64);
    let chained = payload.len() > 4076;
    let mut descriptor = ((if chained { 0 } else { 0x40000000 }) | payload.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend((LVAL << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let added = frames::Row {
        id: 10,
        group: Some(100),
        binary: Some(descriptor),
    };
    let mut bytes = frames::expected(case, &[added]);
    if chained {
        assert!(payload.len() <= 8144);
        bytes.resize((EOF as usize + 1) * PAGE, 0);
        let mut first = (EOF << 8).to_le_bytes().to_vec();
        first.extend(&payload[..4072]);
        let mut last = vec![0; 4];
        last.extend(&payload[4072..]);
        frames::install(&mut bytes, LVAL, &lval_page(Some(first)));
        frames::install(&mut bytes, EOF, &lval_page(Some(last)));
        column_maps(&mut bytes, &[LVAL, EOF], &[EOF]);
        frames::map_page(&mut bytes, 1, vec![frames::map(EOF + 1..512)]);
    } else {
        frames::install(&mut bytes, LVAL, &lval_page(Some(payload)));
        column_maps(&mut bytes, &[LVAL], &[LVAL]);
    }
    bytes
}

fn logical(family: Family, added: Option<&Value>) -> Vec<Vec<Value>> {
    let values = if matches!(family, Family::Ole) {
        vec![
            Value::Null,
            Value::Binary(vec![]),
            Value::Binary(vec![0, 128, 255]),
        ]
    } else {
        vec![
            Value::Null,
            Value::Text(String::new()),
            Value::Text("ABC".into()),
        ]
    };
    let mut rows = values
        .into_iter()
        .enumerate()
        .map(|(position, value)| {
            let id = position as i32 + 1;
            vec![Value::Int32(id), Value::Int32(id * 10), value]
        })
        .collect::<Vec<_>>();
    if let Some(value) = added {
        rows.push(vec![Value::Int32(10), Value::Int32(100), value.clone()]);
    }
    rows
}

fn cases() -> Vec<(Family, Value)> {
    vec![
        (Family::Ole, Value::Binary(vec![0x9d; 65])),
        (Family::Ole, Value::Binary(vec![0x9d; 4076])),
        (Family::Ole, Value::Binary(vec![0x9d; 4077])),
        (Family::Memo, Value::Text("Ā".repeat(33))),
        (Family::Memo, Value::Text("Ā".repeat(2038))),
        (Family::Memo, Value::Text("Ā".repeat(2039))),
    ]
}

#[test]
fn external_sources_and_expected_images_have_real_column_maps_and_exact_chains() {
    for code in [5, 6] {
        for (family, value) in cases() {
            let case = original(code, family);
            checked(&case.before, &logical(family, None));
            check_lval(&case.before);
            let expected = expected(&case, family, &value);
            checked(&expected, &logical(family, Some(&value)));
            check_lval(&expected);
        }
    }
}

#[test]
fn external_single_and_chained_payloads_match_complete_images_without_rewriting_old_rows() {
    for code in [5, 6] {
        for (family, value) in cases() {
            let case = original(code, family);
            checked(&case.before, &logical(family, None));
            check_lval(&case.before);
            let expected = expected(&case, family, &value);
            checked(&expected, &logical(family, Some(&value)));
            check_lval(&expected);
            let request = RowInsert::new(vec![Value::Int32(10), Value::Int32(100), value]);
            let candidate = original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &[request], &mut original::budget(), &|| false)
                .unwrap();
            let actual = candidate.candidate_bytes();
            assert_eq!(actual.len(), expected.len());
            let difference = actual
                .iter()
                .zip(&expected)
                .position(|(actual, expected)| actual != expected);
            assert!(
                difference.is_none(),
                "first external LVAL image difference: {difference:?}"
            );
            check_lval(actual);
        }
    }
}

#[test]
fn external_payloads_honor_each_pool_and_terminal_cancellation_without_a_candidate() {
    let case = original(6, Family::Ole);
    let value = Value::Binary(vec![0x9d; 4077]);
    let expected = expected(&case, Family::Ole, &value);
    let request = [RowInsert::new(vec![
        Value::Int32(10),
        Value::Int32(100),
        value,
    ])];
    let polls = Cell::new(0_usize);
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &request, &mut original::budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), expected);
    let terminal = polls.get();
    assert!(terminal > 1);

    let observed = Cell::new(0_usize);
    let cancelled = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &request, &mut original::budget(), &|| {
            observed.set(observed.get() + 1);
            observed.get() >= terminal
        })
        .err();
    assert_eq!(cancelled, Some(EditError::Inspect(InspectError::Cancelled)));
    assert_eq!(observed.get(), terminal);

    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        let error = original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &request, &mut budget, &|| false)
            .err();
        assert!(matches!(
            error,
            Some(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
}
