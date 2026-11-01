//! Source-only qualification; no fitted planner, preview or mutation executes.
use crate::{NativeImage, check_table, check_table_constraints, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_ordinary_fitted::{self as model, Case, Recipe};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(127);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn qualify(bytes: &[u8], rows: &[Vec<Value>], header: u8) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(checked.indexes().is_empty());
    let schema = checked.schema();
    assert_eq!(schema.definition_page(), 7);
    assert_eq!(schema.columns().len(), 8);
    let expected_names = [
        "Id",
        "TextValue",
        "BinaryValue",
        "DecimalValue",
        "Flag",
        "Large",
        "Money",
        "Date",
    ];
    let native = [4, 10, 9, 16, 1, 19, 5, 8];
    for (number, field) in schema.columns().iter().enumerate() {
        assert_eq!(field.name(), expected_names[number]);
        assert_eq!(field.number(), number);
        assert_eq!(field.native_type(), native[number]);
        assert_eq!(field.extended_flags(), u8::from(number == 1));
    }
    let actual = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(actual.rows(), rows);
    check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    // Existing read-only full ordinary closure and eligible fixed sibling
    // preflight, not a mutation or preview and not a reusable permission.
    image
        .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(&bytes[20..24], &u32::from(header).to_le_bytes());
    assert_eq!(image.as_bytes(), bytes);
}

fn raw(case: &Case) {
    assert_eq!(case.before.len(), 9 * PAGE);
    let td = &case.before[7 * PAGE..8 * PAGE];
    assert_eq!(&td[55..63], &[3, 3, 0, 0, 7, 3, 0, 0]);
    assert_eq!(td[63 + 25 + 16], 1);
    assert_eq!(&td[63 + 25 + 11..63 + 25 + 15], &[9, 4, 0, 1]);
    let data = &case.before[8 * PAGE..9 * PAGE];
    assert_eq!(&data[..2], &[1, 1]);
    assert_eq!(&data[4..8], &[7, 0, 0, 0]);
    assert_eq!(word(data, 2), 3900);
    assert_eq!(word(data, 12), 3);
    for (row, frame) in case.frames.iter().enumerate() {
        assert_eq!(word(data, 14 + row * 2), [4016, 3963, 3920][row]);
        assert_eq!(frame.absolute, 8 * PAGE + [4016, 3963, 3920][row]);
        assert_eq!(frame.length, [80, 53, 43][row]);
        assert_eq!(
            frame.starts,
            [[30, 40, 44, 61], [30, 30, 30, 30], [30, 32, 32, 32]][row]
        );
        let original = &case.before[frame.absolute..frame.absolute + frame.length];
        assert_eq!(original[frame.length - 1], [0xef, 0x11, 0xe7][row]);
        assert!(
            original[frame.starts[3]..frame.data_end]
                .iter()
                .all(|byte| *byte == 0xa5)
        );
        if row == 1 {
            assert_eq!(&original[6..14], &[0xa7; 8]);
            assert_eq!(&original[14..22], &[0xce; 8]);
            assert_eq!(&original[22..30], &[0xd0; 8]);
        }
    }
    model::assert_complement(case);
    if let Some(after) = &case.expected {
        for frame in &case.frames {
            let record = &after[frame.absolute..frame.absolute + frame.length];
            let offsets = (0..4)
                .map(|slot| word(record, frame.length - 5 - slot * 2))
                .collect::<Vec<_>>();
            assert_eq!(offsets[0], 30);
            assert!(offsets.windows(2).all(|pair| pair[0] <= pair[1]));
            assert!(offsets[3] <= frame.data_end);
            assert_eq!(word(record, frame.length - 3), 3);
            let original = &case.before[frame.absolute..frame.absolute + frame.length];
            assert_eq!(
                &record[offsets[3]..frame.data_end],
                &original[offsets[3]..frame.data_end]
            );
            let decimal = &record[offsets[2]..offsets[3]];
            assert_eq!(decimal, &original[frame.starts[2]..frame.starts[3]]);
        }
    }
}

#[test]
fn ordinary_fitted_sources_and_literal_outcomes_qualify_without_writers() {
    let mut views = (0, 0);
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            raw(&case);
            qualify(&case.before, &case.rows, header);
            views.0 += 1;
            if let Some(after) = &case.expected {
                qualify(after, &case.final_rows, header);
                assert!(case.refusal.is_none());
                if matches!(
                    recipe,
                    Recipe::ExactNoops | Recipe::NullNoops | Recipe::EmptyGroups
                ) {
                    assert_eq!(case.changed, 0);
                    assert_eq!(after, &case.before);
                } else {
                    assert_eq!(
                        case.changed,
                        if recipe == Recipe::DisjointRows { 2 } else { 1 }
                    );
                }
                views.1 += 1;
            } else {
                assert!(case.refusal.is_some());
            }
        }
        let forward = model::source(header, Recipe::SharedExactFit);
        let reversed = model::source(header, Recipe::SharedReversed);
        assert_eq!(forward.expected, reversed.expected);
        let frame = &forward.frames[0];
        let record =
            &forward.expected.as_ref().unwrap()[frame.absolute..frame.absolute + frame.length];
        assert_eq!(word(record, frame.length - 11), frame.data_end);
    }
    assert_eq!(views, (38, 30));
}
