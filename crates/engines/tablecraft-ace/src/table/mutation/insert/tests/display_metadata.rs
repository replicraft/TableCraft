//! Literal presentation properties do not change raw stored scalar values.
mod api;

use super::{
    incrementing_autonumber::model as generated,
    model as original,
    ordinary_rows::{fixture, model::Row},
    passive_defaults::model as properties,
};
use crate::{RuleCheckStatus, check_table_constraints, inspect_table_properties, read_table};
use properties::{Group, PAGE, Property, field};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

fn decimal_places(value: u8, flags: u8) -> Property {
    Property {
        name: "DecimalPlaces",
        kind: 2,
        flags,
        data: vec![value],
    }
}

fn format(value: &str, flags: u8) -> Property {
    Property {
        flags,
        ..Property::text("Format", value)
    }
}

fn source(code: u8, places: u8, flags: u8) -> fixture::Case {
    let mut case = fixture::source(code, true, false, fixture::Index::Alias, false);
    let mut groups = [
        "Earlier", "Tiny", "Small", "Large", "Money", "Ratio", "Measure",
    ]
    .into_iter()
    .map(|name| {
        field(
            name,
            vec![
                format("0.000000000000000", flags),
                decimal_places(places, flags),
            ],
        )
    })
    .collect::<Vec<_>>();
    groups.extend([
        field("Flag", vec![format("Yes/No", flags)]),
        field("When", vec![format("yyyy-mm-dd hh:nn:ss", flags)]),
        field(
            "LaterText",
            vec![Property::boolean("AllowZeroLength", true)],
        ),
        field(
            "Score",
            vec![
                format(">", flags),
                Property::boolean("AllowZeroLength", true),
            ],
        ),
    ]);
    properties::properties(&mut case.before, &groups);
    case
}

fn additions() -> [Row; 3] {
    let mut value = Row::rich(10);
    value.score = Some("mIxEd".to_owned());
    value.money = Some(123456789);
    let null = Row::nulls(11);
    let mut empty = Row::rich(12);
    empty.score = Some(String::new());
    [value, null, empty]
}

fn checked(bytes: &[u8], rows: usize) {
    original::checked(bytes, rows, 2);
    checked_constraints(bytes);
}

fn checked_constraints(bytes: &[u8]) {
    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    for field in report.fields() {
        assert!(
            ![field.required(), field.empty_text(), field.validation()]
                .contains(&RuleCheckStatus::Unsupported)
        );
    }
}

fn payload_start(bytes: &[u8], ordinal: usize) -> usize {
    let slot = 12 * PAGE + 14 + 2 * ordinal;
    12 * PAGE + usize::from(u16::from_le_bytes(bytes[slot..slot + 2].try_into().unwrap()) & 0x1fff)
}

fn fixed_expected(case: &fixture::Case) -> Vec<u8> {
    let mut expected = case.before.clone();
    let start = payload_start(&expected, 0) + 2;
    expected[start..start + 4].copy_from_slice(&777_i32.to_le_bytes());
    expected
}

fn text_expected(case: &fixture::Case) -> Vec<u8> {
    let mut expected = case.before.clone();
    // Independent row owner: fixed bytes end at 70, FirstBytes occupies 3 bytes,
    // then Score is raw UTF16LE. Its replacement has exactly the same width.
    let start = payload_start(&expected, 0) + 73;
    assert_eq!(&expected[start..start + 4], &[65, 0, 66, 0]);
    expected[start..start + 4].copy_from_slice(&[109, 0, 73, 0]);
    expected
}

fn delete_expected(case: &fixture::Case) -> Vec<u8> {
    let mut expected = case.before.clone();
    let slot = 12 * PAGE + 14;
    let word = u16::from_le_bytes(expected[slot..slot + 2].try_into().unwrap());
    expected[slot..slot + 2].copy_from_slice(&(word | 0xc000).to_le_bytes());
    expected[10 * PAGE + 16..10 * PAGE + 20].copy_from_slice(&2_u32.to_le_bytes());

    let root = 13 * PAGE;
    assert_eq!(&expected[root + 24..root + 26], &[0, 0]);
    assert_eq!(
        &expected[root + 480..root + 489],
        &[127, 128, 0, 0, 1, 0, 0, 12, 0]
    );
    // Surviving logical ROWs keep their physical slot references. The original
    // inactive key tail, deleted record body, historical count and maps survive.
    expected.copy_within(root + 489..root + 507, root + 480);
    expected[root + 27..root + 480].fill(0);
    for end in [9, 18] {
        expected[root + 27 + end / 8] |= 1 << (end % 8);
    }
    expected[root + 2..root + 4].copy_from_slice(&3598_u16.to_le_bytes());
    expected
}

fn negative_sources() -> Vec<Vec<u8>> {
    let mut groups = vec![
        Group {
            kind: 0,
            name: "",
            properties: vec![format("0.00", 0)],
        },
        Group {
            kind: 2,
            name: "PrimaryID",
            properties: vec![format("0.00", 0)],
        },
        field("Missing", vec![format("0.00", 0)]),
        field("Earlier", vec![format("0.00", 2)]),
        field("Earlier", vec![Property::boolean("Format", false)]),
        field("Earlier", vec![Property::text("DecimalPlaces", "2")]),
        field("Earlier", vec![decimal_places(16, 0)]),
        field("Earlier", vec![decimal_places(254, 0)]),
        field("Earlier", vec![decimal_places(2, 2)]),
        field("Score", vec![decimal_places(2, 0)]),
        field("When", vec![decimal_places(2, 0)]),
        field("Flag", vec![decimal_places(2, 0)]),
        field("Token", vec![format("0.00", 0)]),
        field("FirstBytes", vec![format("0.00", 0)]),
    ];
    for name in ["InputMask", "UnicodeCompression", "RowSource", "NewValues"] {
        groups.push(field("Earlier", vec![Property::text(name, "0")]));
    }

    groups
        .into_iter()
        .map(|group| {
            let mut case = fixture::source(6, true, false, fixture::Index::Alias, false);
            properties::properties(&mut case.before, &[group]);
            case.before
        })
        .collect()
}

fn generated_source(code: u8) -> generated::Case {
    let mut case = generated::source(code, generated::State::HighWater);
    properties::properties(
        &mut case.before,
        &[
            field("ID", vec![format("000000", 1)]),
            field("Score", vec![format("0.00", 1), decimal_places(2, 1)]),
        ],
    );
    case
}

fn generated_addition() -> generated::Row {
    generated::Row {
        earlier: 4,
        id: 261,
        score: None,
    }
}

#[test]
fn qualify_passive_display_incrementing_source_and_complete_expected_before_api() {
    for code in [5, 6] {
        let case = generated_source(code);
        let expected = generated::expected(&case, &[generated_addition()]);
        original::checked(&case.before, 2, 4);
        original::checked(&expected, 3, 4);
        checked_constraints(&case.before);
        checked_constraints(&expected);
        assert_eq!(
            &expected[4 * PAGE..5 * PAGE],
            &case.before[4 * PAGE..5 * PAGE]
        );
        let snapshot = read_table(
            &mut Cursor::new(&expected),
            CatalogScope::new(1),
            "Items",
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(snapshot.rows()[2], generated_addition().values());
    }
}

#[test]
fn qualify_passive_display_sources_and_complete_expected_images_before_api() {
    for code in [5, 6] {
        for places in [0, 2, 15, 255] {
            for flags in [0, 1] {
                let case = source(code, places, flags);
                let additions = additions();
                let inserted = fixture::expected(&case, &additions);
                let fixed = fixed_expected(&case);
                let text = text_expected(&case);
                let deleted = delete_expected(&case);
                for (bytes, rows) in [
                    (&case.before, 3),
                    (&inserted, 6),
                    (&fixed, 3),
                    (&text, 3),
                    (&deleted, 2),
                ] {
                    checked(bytes, rows);
                    assert_eq!(&bytes[4 * PAGE..5 * PAGE], &case.before[4 * PAGE..5 * PAGE]);
                }

                let snapshot = read_table(
                    &mut Cursor::new(&inserted),
                    CatalogScope::new(1),
                    "Items",
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    &snapshot.rows()[3..],
                    additions
                        .iter()
                        .map(|row| row.values(true))
                        .collect::<Vec<_>>()
                );
                let properties = inspect_table_properties(
                    &mut Cursor::new(&case.before),
                    CatalogScope::new(1),
                    "Items",
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
                let numeric = properties
                    .groups()
                    .iter()
                    .find(|group| group.name() == "Earlier")
                    .unwrap();
                let decimal = numeric.get("DecimalPlaces").unwrap();
                assert_eq!(decimal.native_type(), 2);
                assert_eq!(decimal.raw_value(), &[places]);
                assert_eq!(decimal.flags(), flags);
                assert_eq!(numeric.get("Format").unwrap().native_type(), 10);
            }
        }
    }

    // These declarations are readable metadata; their mutation interpretation is
    // deliberately unqualified. No API is invoked by this source-only gate.
    for bytes in negative_sources() {
        original::checked(&bytes, 3, 2);
        inspect_table_properties(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
    }
}
