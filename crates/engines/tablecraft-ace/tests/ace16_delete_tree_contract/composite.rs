use super::{model::*, numeric_fixture, support};
use std::io::Cursor;
use tablecraft_ace::{RowDelete, check_table, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

fn key(score: Option<i32>, secondary: Option<i32>, row: usize, descending: bool) -> Vec<u8> {
    let mut bytes = entry(score, row, descending);
    bytes.truncate(bytes.len() - 4);
    bytes.extend(entry(secondary, row, !descending));
    bytes
}

#[test]
fn composite_separator_maxima_preserve_full_nullable_components_and_row_ties() {
    let keys = [
        (None, None),
        (None, None),
        (Some(10), None),
        (Some(10), Some(1)),
        (Some(10), Some(1)),
        (Some(10), Some(2)),
        (Some(20), Some(0)),
        (Some(30), Some(3)),
    ];
    let source = keys
        .iter()
        .enumerate()
        .map(|(row, &(score, secondary))| {
            vec![
                Some(row as i64 + 1),
                score.map(i64::from),
                secondary.map(i64::from),
            ]
        })
        .collect::<Vec<_>>();
    for descending in [false, true] {
        let mut bytes = numeric_fixture::database(
            6,
            &[4, 4, 4],
            &[(1, descending), (2, !descending)],
            &source,
            0,
        );
        bytes.resize(20 * 4096, 0);
        // Independently authored three-field metadata stores its physical root
        // at212 and usage reference at208. Keep the original logical identity.
        support::put32(&mut bytes, 10 * 4096 + 212, 13);
        support::put32(&mut bytes, 10 * 4096 + 208, 14 << 8);
        support::put32(&mut bytes, 10 * 4096 + 67, 8);
        bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(
            1,
            &[(0, vec![0, 0, 0, 0, 0, 0, 0xa0, 9])],
        ));
        let mut ordered = (0..8).collect::<Vec<_>>();
        ordered.sort_by(|&left, &right| {
            let first = keys[left].0.cmp(&keys[right].0);
            let first = if descending { first.reverse() } else { first };
            let second = keys[left].1.cmp(&keys[right].1);
            let second = if descending { second } else { second.reverse() };
            first.then(second).then(left.cmp(&right))
        });
        let leaves = [
            (15_u32, &ordered[..2]),
            (16, &ordered[2..5]),
            (19, &ordered[5..]),
        ];
        let mut bounds = Vec::new();
        for (position, &(number, rows)) in leaves.iter().enumerate() {
            let entries = rows
                .iter()
                .map(|&row| key(keys[row].0, keys[row].1, row, descending))
                .collect::<Vec<_>>();
            let previous = position.checked_sub(1).map_or(0, |i| leaves[i].0);
            let next = leaves.get(position + 1).map_or(0, |leaf| leaf.0);
            bytes[number as usize * 4096..(number as usize + 1) * 4096]
                .copy_from_slice(&page(0, &entries, previous, next, 0));
            let mut bound = entries.last().unwrap().clone();
            bound.extend(number.to_be_bytes());
            bounds.push(bound);
        }
        bytes[13 * 4096..14 * 4096].copy_from_slice(&page(1, &bounds, 0, 0, 0));
        let deleted = leaves
            .iter()
            .map(|leaf| *leaf.1.last().unwrap())
            .collect::<Vec<_>>();
        let requests = deleted
            .iter()
            .map(|&row| {
                let mut expected = vec![3, 0];
                for cell in &source[row] {
                    expected.extend((cell.unwrap_or(0) as i32).to_le_bytes());
                }
                expected.push(
                    1 | (u8::from(keys[row].0.is_some()) << 1)
                        | (u8::from(keys[row].1.is_some()) << 2),
                );
                RowDelete::new(row, expected)
            })
            .collect::<Vec<_>>();
        let candidate = preview(&bytes, &requests).unwrap();
        let after = candidate.candidate_bytes();
        let expected = ordered
            .iter()
            .filter(|row| !deleted.contains(row))
            .map(|row| Value::Int32(*row as i32 + 1))
            .collect::<Vec<_>>();
        let actual = read_table_index(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            "Compound",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            actual
                .rows()
                .iter()
                .map(|row| row[0].clone())
                .collect::<Vec<_>>(),
            expected
        );
        let mut expected_bounds = Vec::new();
        for (number, rows) in leaves {
            let row = rows
                .iter()
                .copied()
                .rfind(|row| !deleted.contains(row))
                .unwrap();
            expected_bounds.extend(key(keys[row].0, keys[row].1, row, descending));
            expected_bounds.extend(number.to_be_bytes());
            let offset = number as usize * 4096;
            assert_eq!(
                &bytes[offset + 4..offset + 27],
                &after[offset + 4..offset + 27]
            );
        }
        assert_eq!(
            &after[13 * 4096 + 480..13 * 4096 + 480 + expected_bounds.len()],
            expected_bounds
        );
        assert_eq!(&bytes[14 * 4096..15 * 4096], &after[14 * 4096..15 * 4096]);
        assert_eq!(
            &bytes[10 * 4096 + 67..10 * 4096 + 71],
            &after[10 * 4096 + 67..10 * 4096 + 71]
        );
        assert_eq!(
            &bytes[13 * 4096 + 4..13 * 4096 + 27],
            &after[13 * 4096 + 4..13 * 4096 + 27]
        );
        assert_eq!(candidate.deleted_rows(), 3);
        check_table(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}
