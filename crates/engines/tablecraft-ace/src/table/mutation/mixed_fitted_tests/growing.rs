//! Independent growing-record sources and complete expected images.
use super::{model, qualify};
use model::{Family, PAGE, Recipe, Row};
use tablecraft_testkit::access as raw;

#[derive(Clone, Copy)]
pub(super) enum Destination {
    OriginalPage,
    OtherOwned,
    EndOfFile,
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub rows: Vec<Row>,
    pub final_rows: Vec<Row>,
    pub family: Family,
    pub target: u32,
}

fn data_page(rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = raw::data_page(10, rows);
    let payload = rows.iter().map(|(_, record)| record.len()).sum::<usize>();
    let free = PAGE - 14 - rows.len() * 2 - payload;
    raw::put16(&mut page, 2, free as u16);
    page
}

fn map_page(owned: &[u32], candidates: &[u32]) -> Vec<u8> {
    let mut records = Vec::new();
    for members in [owned, candidates] {
        let mut bytes = vec![0; 69];
        for &page in members {
            bytes[5 + page as usize / 8] |= 1 << (page % 8);
        }
        records.push((0, bytes));
    }
    let mut page = raw::data_page(0, &records);
    raw::put16(&mut page, 2, (PAGE - 14 - 4 - 138) as u16);
    page
}

fn record_before() -> Vec<u8> {
    let mut record = 4_u16.to_le_bytes().to_vec();
    record.extend(4_i32.to_le_bytes());
    record.extend(0x6655_4433_i32.to_le_bytes());
    record.extend([b'Z', 0, b'4', 0]);
    for offset in [14_u16, 10, 10] {
        record.extend(offset.to_le_bytes());
    }
    record.extend(2_u16.to_le_bytes());
    record.push(9);
    assert_eq!(record.len(), 23);
    record
}

fn record_after(family: Family) -> Vec<u8> {
    let mut record = 4_u16.to_le_bytes().to_vec();
    record.extend(4_i32.to_le_bytes());
    record.extend(40_i32.to_le_bytes());
    let name = match family {
        Family::Binary => vec![0, 255],
        _ => vec![b'a', 0, b'a', 0],
    };
    record.extend(&name);
    record.extend([b'Z', 0, b'4', 0]);
    for offset in [14 + name.len(), 10 + name.len(), 10] {
        record.extend((offset as u16).to_le_bytes());
    }
    record.extend(2_u16.to_le_bytes());
    record.push(15);
    assert_eq!(record.len(), 23 + name.len());
    record
}

pub(super) fn case(code: u8, family: Family, descending: bool, destination: Destination) -> Case {
    let source = model::case(code, family, descending, Recipe::Nulls, false);
    let rows = source.rows;
    let mut final_rows = rows.clone();
    final_rows[3].age = Some(40);
    final_rows[3].name = Some(if matches!(family, Family::Binary) {
        vec![0, 255]
    } else {
        vec![b'a', b'a']
    });

    let mut before = source.before;
    // The read-only donor does not promise complete map declarations. These
    // three independently calculated native name ends have no map pairs.
    for (definition, tail) in [(2, 398), (7, 449), (10, 663)] {
        let at = definition * PAGE + tail;
        assert_eq!(&before[at..at + 2], &[0, 0]);
        before[at..at + 2].copy_from_slice(&[0xff, 0xff]);
    }
    let mut records = source.records;
    records[3] = record_before();
    let mut physical = records
        .iter()
        .cloned()
        .map(|record| (0, record))
        .collect::<Vec<_>>();
    if matches!(destination, Destination::EndOfFile) {
        // Four live records and a genuine deleted slot consume the full page.
        let used = physical
            .iter()
            .map(|(_, record)| record.len())
            .sum::<usize>();
        let deleted_length = PAGE - 14 - 5 * 2 - used;
        physical.push((0x8000, vec![0xbc; deleted_length]));
    }
    before[12 * PAGE..13 * PAGE].copy_from_slice(&data_page(&physical));

    let alternate = matches!(destination, Destination::OtherOwned);
    if alternate {
        before.resize(18 * PAGE, 0);
        before[17 * PAGE..18 * PAGE].copy_from_slice(&data_page(&[]));
    }
    let owned = if alternate { vec![12, 17] } else { vec![12] };
    before[11 * PAGE..12 * PAGE].copy_from_slice(&map_page(&owned, &owned));
    // TD owned reference is 11:0; the distinct candidate record is 11:1.
    raw::put32(&mut before, 10 * PAGE + 59, (11 << 8) | 1);
    if alternate {
        let start = u16::from_le_bytes(before[PAGE + 14..PAGE + 16].try_into().unwrap()) as usize;
        before[PAGE + start + 5 + 17 / 8] &= !(1 << (17 % 8));
    }

    let final_record = record_after(family);
    let mut expected = before.clone();
    let target = if matches!(destination, Destination::OriginalPage) {
        (12_u32 << 8) | 4
    } else {
        17_u32 << 8
    };
    if matches!(destination, Destination::EndOfFile) {
        expected.resize(18 * PAGE, 0);
    }
    if matches!(destination, Destination::OriginalPage) {
        physical.push((0x8000, final_record.clone()));
        expected[12 * PAGE..13 * PAGE].copy_from_slice(&data_page(&physical));
    } else {
        expected[17 * PAGE..18 * PAGE].copy_from_slice(&data_page(&[(0x8000, final_record)]));
    }

    let slot_at = 12 * PAGE + 14 + 3 * 2;
    let start = u16::from_le_bytes(before[slot_at..slot_at + 2].try_into().unwrap()) as usize;
    assert_eq!(
        &before[12 * PAGE + start..12 * PAGE + start + 23],
        records[3].as_slice()
    );
    expected[12 * PAGE + start..12 * PAGE + start + 23].fill(0);
    raw::put32(&mut expected, 12 * PAGE + start, target);
    raw::put16(&mut expected, slot_at, start as u16 | 0x4000);

    if matches!(destination, Destination::EndOfFile) {
        expected[11 * PAGE..12 * PAGE].copy_from_slice(&map_page(&[12, 17], &[17]));
        let start = u16::from_le_bytes(before[PAGE + 14..PAGE + 16].try_into().unwrap()) as usize;
        expected[PAGE + start + 5 + 17 / 8] &= !(1 << (17 % 8));
    }
    for (physical, root) in [13, 15, 16].into_iter().enumerate() {
        let page = model::index_page(
            &before[root * PAGE..(root + 1) * PAGE],
            &final_rows,
            physical,
            family,
            descending,
        );
        expected[root * PAGE..(root + 1) * PAGE].copy_from_slice(&page);
    }

    Case {
        before,
        expected,
        rows,
        final_rows,
        family,
        target,
    }
}

#[test]
fn qualify_growing_mixed_sources_and_complete_expected_before_preview() {
    use crate::NativeImage;
    use crate::table::mutation::{
        gates, page_update::roots, rows::RowAssignment, variable_split::metadata::Metadata,
    };
    use std::io::Cursor;
    use tablecraft_model::CatalogScope;

    struct Selected;
    impl RowAssignment for Selected {
        fn row(&self) -> usize {
            3
        }
    }

    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for destination in [
                    Destination::OriginalPage,
                    Destination::OtherOwned,
                    Destination::EndOfFile,
                ] {
                    let case = case(code, family, descending, destination);
                    qualify(&case.before, &case.rows, case.family);
                    qualify(&case.expected, &case.final_rows, case.family);

                    for (bytes, final_image) in [(&case.before, false), (&case.expected, true)] {
                        for (definition, tail) in [(2, 398), (7, 449), (10, 663)] {
                            let at = definition * PAGE + tail;
                            assert_eq!(&bytes[at..at + 2], &[0xff, 0xff]);
                        }
                        let mut budget = super::budget();
                        let image = NativeImage::capture(
                            &mut Cursor::new(bytes),
                            CatalogScope::new(1),
                            &mut budget,
                            &|| false,
                        )
                        .unwrap();
                        let checked =
                            gates::before_slot(&image, "Items", &mut budget, &|| false).unwrap();
                        Metadata::capture_for_insert(
                            &image,
                            checked.schema(),
                            checked.indexes(),
                            &mut budget,
                            &|| false,
                        )
                        .unwrap();
                        let selected = [Selected];
                        let captured = roots::capture(
                            &image,
                            checked.schema(),
                            &selected,
                            &mut budget,
                            &|| false,
                        )
                        .unwrap();
                        assert_eq!(captured.selected.len(), 1);
                        assert_eq!(captured.selected[0].reference, (12 << 8) | 3);
                        if final_image {
                            assert_eq!(
                                captured.selected[0].final_record.data,
                                record_after(family)
                            );
                            assert_eq!(
                                captured.selected[0].final_record.offset / PAGE,
                                (case.target >> 8) as usize
                            );
                        } else {
                            assert_eq!(captured.selected[0].final_record.data, record_before());
                        }
                    }
                    assert_eq!(
                        case.target >> 8,
                        match destination {
                            Destination::OriginalPage => 12,
                            Destination::OtherOwned | Destination::EndOfFile => 17,
                        }
                    );
                }
            }
        }
    }
}
