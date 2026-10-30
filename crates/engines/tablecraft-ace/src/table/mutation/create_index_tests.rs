//! Qualify independently authored old/new schema images before any DDL producer.
use crate::{
    IndexCheckStatus, NativeImage, check_table, inspect_table_index_counter, read_table,
    table::mutation::append::owners::MultipleOwners,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

mod model;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 16_000_000)
}

fn leaf_entries(page: &[u8]) -> Vec<Vec<u8>> {
    assert_eq!(page[0], 4);
    let prefix = usize::from(u16::from_le_bytes(page[24..26].try_into().unwrap()));
    let active = 3616 - usize::from(u16::from_le_bytes(page[2..4].try_into().unwrap()));
    let ends = (1..=active).filter(|end| page[27 + end / 8] & (1 << (end % 8)) != 0);
    let mut result: Vec<Vec<u8>> = Vec::new();
    let mut previous = 0;
    for end in ends {
        let mut wire = result
            .first()
            .map_or_else(Vec::new, |first| first[..prefix].to_vec());
        wire.extend(&page[480 + previous..480 + end]);
        result.push(wire);
        previous = end;
    }
    assert_eq!(previous, active);
    result
}

fn page(bytes: &[u8], number: u32) -> &[u8] {
    &bytes[number as usize * 4096..(number as usize + 1) * 4096]
}

fn qualify(bytes: &[u8], case: &model::Case, after: bool) {
    let scope = CatalogScope::new(7);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap();
    let table = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.schema().object_id(), scope.object_id(10));
    assert_eq!(table.schema().definition_page(), 10);
    assert_eq!(
        table
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["Id", "Score", "Guard"]
    );
    let expected_rows = case
        .rows
        .iter()
        .map(|row| {
            vec![
                Value::Int32(row.id),
                row.score.map_or(Value::Null, Value::Int32),
                Value::Int32(row.guard),
            ]
        })
        .collect::<Vec<_>>();
    assert_eq!(table.rows(), expected_rows);
    let sentinel = read_table(
        &mut image.reader(),
        scope,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sentinel.rows(), [vec![Value::Int32(999)]]);
    assert_eq!(sentinel.schema().definition_page(), 18);

    let checked = check_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), case.rows.len());
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert_eq!(checked.indexes().len(), if after { 5 } else { 4 });
    for (position, name) in ["PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias"]
        .into_iter()
        .enumerate()
    {
        let index = checked.indexes()[position].index();
        assert_eq!(index.name(), name);
        assert_eq!(index.number(), position as u32);
        assert_eq!(index.data_number(), (position / 2) as u32);
        assert_eq!(index.root_page(), 13 + (position / 2) as u32);
        assert_eq!(index.usage_map(), (15 << 8) | (position / 2) as u32);
        assert_eq!(index.columns().len(), 1);
        assert_eq!(
            index.columns()[0].number(),
            if position < 2 { 0 } else { 2 }
        );
        let counter = inspect_table_index_counter(
            &mut image.reader(),
            scope,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.value(), if position < 2 { 1000 } else { 2000 });
    }
    assert!(leaf_entries(page(bytes, 13)) == case.old_streams[0]);
    assert!(leaf_entries(page(bytes, 14)) == case.old_streams[1]);
    let mut index_pages = vec![13, 14];
    if after {
        let index = checked.indexes()[4].index();
        assert_eq!(index.name(), case.name);
        assert_eq!(index.number(), 4);
        assert_eq!(index.data_number(), 2);
        assert_eq!(index.root_page(), 25);
        assert_eq!(index.usage_map(), 24 << 8);
        assert_eq!(index.native_type(), 0);
        assert_eq!(index.flags(), 0x80);
        assert_eq!(index.columns().len(), 1);
        assert_eq!(index.columns()[0].number(), 1);
        assert_eq!(index.columns()[0].descending(), case.descending);
        let counter = inspect_table_index_counter(
            &mut image.reader(),
            scope,
            "Items",
            case.name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.value(), case.new_counter);
        assert_eq!(counter.byte_range(), 10 * 4096 + 91..10 * 4096 + 95);
        if case.rows.len() == 600 {
            assert_eq!(page(bytes, 25)[0], 3);
            assert_eq!(page(bytes, 25)[26], 1);
            let mut entries = leaf_entries(page(bytes, 26));
            entries.extend(leaf_entries(page(bytes, 27)));
            assert!(entries == case.new_stream);
            index_pages.extend([25, 26, 27]);
        } else {
            assert!(leaf_entries(page(bytes, 25)) == case.new_stream);
            index_pages.push(25);
        }
        assert_eq!(
            bytes.len() / 4096,
            if case.rows.len() == 600 { 28 } else { 26 }
        );
    } else {
        assert_eq!(bytes.len() / 4096, 24);
        assert!(checked.indexes().iter().all(|index| {
            index
                .index()
                .columns()
                .iter()
                .all(|column| column.number() != 1)
        }));
    }
    MultipleOwners::capture(&image, &[&checked], &index_pages, &mut budget(), &|| false).unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_real_secondary_index_source_and_complete_expected_before_writer() {
    let mut source_cases = 0;
    for code in [5, 6] {
        for population in model::POPULATIONS {
            for descending in [false, true] {
                let case = model::source(code, population, descending);
                let expected_counter = match population {
                    model::Population::Empty => 0,
                    model::Population::Distinct => 3,
                    model::Population::NullableDuplicate => 4,
                    model::Population::Pressure => 541,
                };
                assert_eq!(case.new_counter, expected_counter);
                assert_eq!(case.new_stream.len(), case.rows.len());
                qualify(&case.before, &case, false);
                qualify(&case.expected, &case, true);
                let old_definition = page(&case.before, 10);
                let new_definition = page(&case.expected, 10);
                assert_eq!(&old_definition[63..87], &new_definition[63..87]);
                assert_eq!(&old_definition[87..192], &new_definition[99..204]);
                assert_eq!(&old_definition[192..296], &new_definition[204..308]);
                assert_eq!(&old_definition[296..408], &new_definition[360..472]);
                assert_eq!(&old_definition[408..498], &new_definition[500..590]);
                for at in 0..63 {
                    if !(2..4).contains(&at) && !(8..12).contains(&at) && !(47..55).contains(&at) {
                        assert_eq!(
                            old_definition[at], new_definition[at],
                            "unexpected header effect {at}"
                        );
                    }
                }
                let mut expected_global = page(&case.before, 1).to_vec();
                let row = usize::from(u16::from_le_bytes(
                    expected_global[14..16].try_into().unwrap(),
                ));
                for &number in &case.new_pages {
                    expected_global[row + 5 + number as usize / 8] &= !(1 << (number % 8));
                }
                assert!(expected_global == page(&case.expected, 1));
                assert_eq!(case.source_definition_length, 500);
                assert_eq!(
                    case.expected_definition_length,
                    if descending { 624 } else { 622 }
                );
                assert_eq!(&page(&case.before, 10)[498..500], &[0xff, 0xff]);
                assert_eq!(
                    &page(&case.expected, 10)
                        [case.expected_definition_length - 2..case.expected_definition_length],
                    &[0xff, 0xff]
                );
                for number in 0..model::SOURCE_PAGES {
                    if number != 1 && number != 10 {
                        assert!(
                            page(&case.before, number) == page(&case.expected, number),
                            "{population:?}/{code}/{descending}: changed old page {number}"
                        );
                    }
                }
                assert_eq!(
                    &page(&case.expected, 10)[case.expected_definition_length..],
                    &page(&case.before, 10)[case.expected_definition_length..]
                );
                assert_eq!(
                    case.new_pages,
                    if case.rows.len() == 600 {
                        vec![24, 25, 26, 27]
                    } else {
                        vec![24, 25]
                    }
                );
                source_cases += 1;
            }
        }
    }
    assert_eq!(source_cases, 16);
}
