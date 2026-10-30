//! Private independent complete images; no writer, splitter or key table is used.
use super::base;
#[path = "model/data.rs"]
pub(crate) mod data;
#[path = "model/index.rs"]
pub(crate) mod index;
#[path = "model/keys.rs"]
pub(crate) mod keys;
#[path = "model/schema.rs"]
mod schema;

use data::{append_record, data_page, map_page, record};
use index::{index_page, separator};
use keys::entries;
use schema::definition;
use tablecraft_testkit::access as raw;

const PAGE: usize = 4096;
const ROWS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub enum Family {
    Legacy,
    Modern,
    Binary,
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Root,
    Branch,
}

#[derive(Clone)]
pub struct Row {
    pub id: i32,
    pub age: i32,
    pub name: Option<Vec<u8>>,
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub rows: Vec<Row>,
    pub final_rows: Vec<Row>,
    pub family: Family,
    pub data_pages: Vec<u32>,
    pub index_pages: Vec<u32>,
}

pub fn case(code: u8, family: Family, descending: bool, shape: Shape) -> Case {
    let rows = (0..ROWS)
        .map(|ordinal| Row {
            id: ordinal as i32 + 1,
            age: ordinal as i32 + 1,
            name: if ordinal == 0 {
                None
            } else {
                Some(vec![if ordinal % 2 == 0 { b'a' } else { b's' }])
            },
        })
        .collect::<Vec<_>>();
    let mut final_rows = rows.clone();
    let units = if matches!(family, Family::Modern) {
        40
    } else {
        80
    };
    for (ordinal, row) in final_rows.iter_mut().enumerate() {
        row.age += 1000;
        let byte = match family {
            Family::Binary => {
                if ordinal % 2 == 0 {
                    0
                } else {
                    255
                }
            }
            _ => {
                if ordinal % 2 == 0 {
                    b'a'
                } else {
                    b's'
                }
            }
        };
        row.name = Some(vec![byte; units]);
    }
    let mut rows = rows;
    if matches!(family, Family::Binary) {
        for (ordinal, row) in rows.iter_mut().enumerate().skip(1) {
            row.name = Some(vec![if ordinal % 2 == 0 { 0 } else { 255 }]);
        }
    }
    let eof = if matches!(shape, Shape::Root) { 16 } else { 21 };
    let mut before = base::database();
    before.resize(eof * PAGE, 0);
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = code;
    for (td, tail) in [(2, 398), (7, 449)] {
        raw::put16(&mut before, td * PAGE + tail, 0xffff);
    }
    before[10 * PAGE..11 * PAGE].copy_from_slice(&definition(family, descending));
    let records = rows
        .iter()
        .map(|row| (0, record(row, family)))
        .collect::<Vec<_>>();
    before[12 * PAGE..13 * PAGE].copy_from_slice(&data_page(&records));
    before[11 * PAGE..12 * PAGE].copy_from_slice(&map_page(0, &[vec![12], vec![12]]));
    let mut ownership = Vec::new();
    for (physical, root) in [13, 15].into_iter().enumerate() {
        let entries = entries(&rows, family, descending, physical == 1);
        let mut seed = vec![0xa7; PAGE];
        seed[..480].fill(0);
        if matches!(shape, Shape::Root) {
            before[root * PAGE..(root + 1) * PAGE].copy_from_slice(&index_page(
                Some(&seed),
                &entries,
                (0, 0),
                0,
                false,
            ));
            ownership.push(vec![root as u32]);
        } else {
            let left = 16 + physical as u32 * 2;
            let right = left + 1;
            before[left as usize * PAGE..(left as usize + 1) * PAGE].copy_from_slice(&index_page(
                Some(&seed),
                &entries[..48],
                (0, right),
                0,
                false,
            ));
            before[right as usize * PAGE..(right as usize + 1) * PAGE].copy_from_slice(
                &index_page(Some(&seed), &entries[48..], (left, 0), 0, false),
            );
            let bounds = [separator(&entries[47], left)];
            before[root * PAGE..(root + 1) * PAGE].copy_from_slice(&index_page(
                Some(&seed),
                &bounds,
                (0, 0),
                right,
                true,
            ));
            ownership.push(vec![root as u32, left, right]);
        }
    }
    before[14 * PAGE..15 * PAGE].copy_from_slice(&map_page(0, &ownership));
    if matches!(shape, Shape::Branch) {
        before[20 * PAGE..21 * PAGE].fill(0xcc);
    }
    before[PAGE..2 * PAGE].copy_from_slice(&map_page(1, &[(eof as u32..512).collect()]));
    let mut expected = before.clone();
    let mut owned = vec![12];
    let mut hints = vec![12];
    let mut candidate = 12;
    for (ordinal, row) in final_rows.iter().enumerate() {
        append_record(
            &mut expected,
            ordinal,
            &record(row, family),
            &mut candidate,
            &mut owned,
            &mut hints,
        );
    }
    let data_pages = owned[1..].to_vec();
    expected[11 * PAGE..12 * PAGE].copy_from_slice(&map_page(0, &[owned, hints]));
    let mut index_pages = Vec::new();
    for (physical, root) in [13, 15].into_iter().enumerate() {
        let entries = entries(&final_rows, family, descending, physical == 1);
        if matches!(shape, Shape::Root) {
            let right = (expected.len() / PAGE) as u32;
            let left = right + 1;
            expected.extend(index_page(None, &entries[32..], (left, 0), 0, false));
            expected.extend(index_page(None, &entries[..32], (0, right), 0, false));
            let bounds = [separator(&entries[31], left)];
            let original = &before[root * PAGE..(root + 1) * PAGE];
            expected[root * PAGE..(root + 1) * PAGE].copy_from_slice(&index_page(
                Some(original),
                &bounds,
                (0, 0),
                right,
                true,
            ));
            ownership[physical].extend([right, left]);
            index_pages.extend([right, left]);
        } else {
            let left = 16 + physical as u32 * 2;
            let right = left + 1;
            let middle = (expected.len() / PAGE) as u32;
            expected.extend(index_page(None, &entries[..24], (0, left), 0, false));
            let old_left = &before[left as usize * PAGE..(left as usize + 1) * PAGE];
            let old_right = &before[right as usize * PAGE..(right as usize + 1) * PAGE];
            expected[left as usize * PAGE..(left as usize + 1) * PAGE].copy_from_slice(
                &index_page(Some(old_left), &entries[24..48], (middle, right), 0, false),
            );
            expected[right as usize * PAGE..(right as usize + 1) * PAGE].copy_from_slice(
                &index_page(Some(old_right), &entries[48..], (left, 0), 0, false),
            );
            let bounds = [
                separator(&entries[23], middle),
                separator(&entries[47], left),
            ];
            let original = &before[root * PAGE..(root + 1) * PAGE];
            expected[root * PAGE..(root + 1) * PAGE].copy_from_slice(&index_page(
                Some(original),
                &bounds,
                (0, 0),
                right,
                true,
            ));
            ownership[physical].push(middle);
            index_pages.push(middle);
        }
    }
    expected[14 * PAGE..15 * PAGE].copy_from_slice(&map_page(0, &ownership));
    let end = (expected.len() / PAGE) as u32;
    expected[PAGE..2 * PAGE].copy_from_slice(&map_page(1, &[(end..512).collect()]));
    Case {
        before,
        expected,
        rows,
        final_rows,
        family,
        data_pages,
        index_pages,
    }
}
