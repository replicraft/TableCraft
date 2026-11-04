use super::{query, request, stored::opened};
use crate::{CellKind, Command, DisplayControl, Response};
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation::{self, Case},
    data_page, mutation_fixture,
};

#[test]
fn native_and_query_booleans_keep_typed_facts_without_text_or_numeric_coercion() {
    let source = canonical_fixed_creation::source(Case::Endpoints);
    let (_directory, path, worker, _revision) = opened(&source.bytes);
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: source.table.into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected native page");
    };
    for (row, expected) in rows.iter().zip([true, false, true]) {
        assert_eq!(
            row[0].kind,
            CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
                boolean: Some(expected),
                display_control: crate::DisplayControl::Absent
            }
        );
        assert_eq!(row[0].text, expected.to_string());
        for cell in &row[1..] {
            assert!(matches!(cell.kind, CellKind::Scalar { boolean: None, .. }));
        }
    }
    assert_eq!(rows.len(), 3);

    let Response::Data { rows, .. } = request(
        &worker,
        query("SELECT True AS [Yes], False AS [No], NULL AS [Absent], 'true' AS [Text], 0 AS [Zero], -1 AS [MinusOne]"),
    )
    .unwrap() else {
        panic!("expected query data");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 6);
    for (index, (boolean, is_null)) in [
        (Some(true), false),
        (Some(false), false),
        (None, true),
        (None, false),
        (None, false),
        (None, false),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            rows[0][index].kind,
            CellKind::Scalar {
                is_null,
                stored_calculation: false,
                boolean,
                display_control: crate::DisplayControl::Uninspected,
            }
        );
    }
    assert_eq!(rows[0][0].text, rows[0][3].text);
    assert_eq!(std::fs::read(path).unwrap(), source.bytes);
}

// Reuse the independent native catalog/property builder and literal DATA-page
// builder. Only the Score declaration, its two mask-only values and the named
// property's exact flags/type change; no production property encoder is used.
fn control_source(key: &str, flags: u8, native_type: u8, text: &str, boolean: bool) -> Vec<u8> {
    const PAGE: usize = 4096;
    let mut bytes = mutation_fixture::with_rule(key, text);
    let payload = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut original = u16::try_from(8 + payload.len())
        .unwrap()
        .to_le_bytes()
        .to_vec();
    original.extend_from_slice(&[0, 10, 0, 0]);
    original.extend_from_slice(&u16::try_from(payload.len()).unwrap().to_le_bytes());
    original.extend_from_slice(&payload);
    let positions = bytes
        .windows(original.len())
        .enumerate()
        .filter_map(|(position, raw)| (raw == original).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1, "one original Score property");
    bytes[positions[0] + 2] = flags;
    bytes[positions[0] + 3] = native_type;

    if boolean {
        let definition =
            mutation_fixture::definition(&[("Id", 4, 0, 4), ("Score", 1, 0, 0)], 2, 11 << 8);
        bytes[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
        // BOOLEAN owns native mask bit1 rather than an independent payload.
        let rows = [(1_i32, 1_u8), (2_i32, 3_u8)]
            .into_iter()
            .map(|(id, mask)| {
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend_from_slice(&id.to_le_bytes());
                row.push(mask);
                (0, row)
            })
            .collect::<Vec<_>>();
        bytes[12 * PAGE..13 * PAGE].copy_from_slice(&data_page(10, &rows));
    }
    bytes
}

#[test]
fn native_control_metadata_is_explicit_and_never_coerces_scalar_values() {
    for (key, flags, native_type, text, boolean, expected) in [
        (
            "DisplayControl",
            1,
            3,
            "j",
            true,
            DisplayControl::Native(106),
        ),
        (
            "DisplayControl",
            1,
            3,
            "m",
            true,
            DisplayControl::Native(109),
        ),
        (
            "DisplayControl",
            1,
            3,
            "z",
            true,
            DisplayControl::Native(122),
        ),
        (
            "DisplayControl",
            2,
            3,
            "j",
            true,
            DisplayControl::Unsupported,
        ),
        (
            "DisplayControl",
            1,
            10,
            "j",
            true,
            DisplayControl::Unsupported,
        ),
        (
            "DisplayControl",
            1,
            3,
            "jk",
            true,
            DisplayControl::Unsupported,
        ),
        ("Other", 1, 3, "j", true, DisplayControl::Absent),
        (
            "DisplayControl",
            1,
            3,
            "j",
            false,
            DisplayControl::Native(106),
        ),
    ] {
        let bytes = control_source(key, flags, native_type, text, boolean);
        let (_directory, path, worker, _revision) = opened(&bytes);
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected inspected native page");
        };
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row[1].kind,
                CellKind::Scalar {
                    is_null: false,
                    stored_calculation: false,
                    boolean: boolean.then_some(index != 0),
                    display_control: expected,
                }
            );
            assert!(matches!(
                row[0].kind,
                CellKind::Scalar {
                    boolean: None,
                    display_control: DisplayControl::Absent,
                    ..
                }
            ));
        }
        assert_eq!(rows.len(), if boolean { 2 } else { 3 });
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn present_malformed_properties_fail_the_page_instead_of_losing_metadata() {
    let mut bytes = control_source("DisplayControl", 1, 3, "j", true);
    let start = bytes.windows(4).position(|raw| raw == b"MR2\0").unwrap();
    bytes[start] = b'X';
    let (_directory, _path, worker, _revision) = opened(&bytes);
    assert!(
        request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0
            }
        )
        .is_err()
    );
}

#[test]
fn absent_catalog_property_carrier_remains_uninspected_without_hiding_errors() {
    let mut bytes = control_source("DisplayControl", 1, 3, "j", true);
    let original = "LvProp"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let renamed = "Unused"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(original.len(), renamed.len());
    let positions = bytes
        .windows(original.len())
        .enumerate()
        .filter_map(|(position, raw)| (raw == original).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(
        positions.len(),
        1,
        "one catalog property-column declaration"
    );
    let start = positions[0];
    bytes[start..start + original.len()].copy_from_slice(&renamed);

    let (_directory, path, worker, _revision) = opened(&bytes);
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected native page without a property carrier");
    };
    assert_eq!(rows.len(), 2);
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(
            row[1].kind,
            CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
                boolean: Some(index != 0),
                display_control: crate::DisplayControl::Uninspected,
            }
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
