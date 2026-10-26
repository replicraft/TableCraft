//! Literal synthetic packets, independent of the production tree writer.
use super::{index_fixture, support};
use std::io::Cursor;
use tablecraft_ace::{
    Ace16DeletePreview, EditError, NativeImage, RowDelete, check_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

pub struct Model {
    pub bytes: Vec<u8>,
    pub keys: Vec<Option<i32>>,
    pub ordered: Vec<usize>,
    pub leaves: Vec<(u32, Vec<usize>)>,
    pub nodes: Vec<(u32, Vec<u32>, bool)>,
    pub descending: bool,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 20_000_000)
}

pub fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub fn preview(bytes: &[u8], requests: &[RowDelete]) -> Result<Ace16DeletePreview, EditError> {
    capture(bytes).preview_ace16_deletes("Items", requests, &mut budget(), &|| false)
}

pub fn request(model: &Model, row: usize) -> RowDelete {
    let mut bytes = vec![2, 0];
    bytes.extend((row as i32 + 1).to_le_bytes());
    bytes.extend(model.keys[row].unwrap_or(0).to_le_bytes());
    bytes.push(if model.keys[row].is_some() { 3 } else { 1 });
    RowDelete::new(row, bytes)
}

pub fn entry(key: Option<i32>, slot: usize, descending: bool) -> Vec<u8> {
    let mut bytes = vec![match (key, descending) {
        (None, false) => 0,
        (None, true) => 255,
        (Some(_), false) => 127,
        (Some(_), true) => 128,
    }];
    if let Some(key) = key {
        let signed = ((key as u32) ^ 0x80000000).to_be_bytes();
        bytes.extend(signed.map(|byte| if descending { !byte } else { byte }));
    }
    bytes.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
    bytes
}

pub fn page(height: u8, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut bytes = vec![0xa7; 4096];
    bytes[..480].fill(0);
    bytes[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    bytes[26] = height;
    support::put32(&mut bytes, 4, 10);
    support::put32(&mut bytes, 12, previous);
    support::put32(&mut bytes, 16, next);
    support::put32(&mut bytes, 20, tail);
    let mut end = 0;
    for entry in entries {
        bytes[480 + end..480 + end + entry.len()].copy_from_slice(entry);
        end += entry.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut bytes, 2, (3616 - end) as u16);
    bytes
}

pub fn database(code: u8, descending: bool, bounded: bool, deep: bool, alias: bool) -> Model {
    let keys = vec![
        None,
        None,
        None,
        Some(10),
        Some(10),
        Some(10),
        Some(20),
        Some(30),
    ];
    let mut ordered = (0..keys.len()).collect::<Vec<_>>();
    ordered.sort_by(|&left, &right| {
        let order = keys[left].cmp(&keys[right]);
        (if descending { order.reverse() } else { order }).then(left.cmp(&right))
    });
    let leaves = vec![
        (15, ordered[..2].to_vec()),
        (16, ordered[2..5].to_vec()),
        (19, ordered[5..].to_vec()),
    ];
    let nodes = if deep {
        vec![
            (13, vec![17, 18], bounded),
            (17, vec![15, 16], bounded),
            (18, vec![19], bounded),
        ]
    } else {
        vec![(13, vec![15, 16, 19], bounded)]
    };
    let mut bytes = index_fixture::database(descending, alias, true);
    bytes.resize(20 * 4096, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    support::put32(&mut bytes, 10 * 4096 + 16, 8);
    support::put32(&mut bytes, 10 * 4096 + 67, 8);
    bytes[10 * 4096 + 189] = 0x80;
    bytes[10 * 4096 + 195 + 23] = 0;
    if alias {
        bytes[10 * 4096 + 195 + 28 + 23] = 0;
    }
    let rows = keys
        .iter()
        .enumerate()
        .map(|(row, key)| {
            let mut bytes = vec![2, 0];
            bytes.extend((row as i32 + 1).to_le_bytes());
            bytes.extend(key.unwrap_or(0_i32).to_le_bytes());
            bytes.push(if key.is_some() { 3 } else { 1 });
            (0, bytes)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    let map = if deep { 15 } else { 9 };
    bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xa0, map])],
    ));
    for (position, (number, rows)) in leaves.iter().enumerate() {
        let entries = rows
            .iter()
            .map(|&row| entry(keys[row], row, descending))
            .collect::<Vec<_>>();
        let previous = position.checked_sub(1).map_or(0, |index| leaves[index].0);
        let next = leaves.get(position + 1).map_or(0, |leaf| leaf.0);
        bytes[*number as usize * 4096..(*number as usize + 1) * 4096]
            .copy_from_slice(&page(0, &entries, previous, next, 0));
    }
    // Construct bottom-up so each bound is the full authored descendant maximum.
    for &(number, ref children, bounded) in nodes.iter().rev() {
        let mut entries = Vec::new();
        for child in children.iter().take(if bounded {
            children.len()
        } else {
            children.len() - 1
        }) {
            let row = subtree_rows(*child, &leaves, &nodes)
                .last()
                .copied()
                .unwrap();
            let mut bound = entry(keys[row], row, descending);
            bound.extend(child.to_be_bytes());
            entries.push(bound);
        }
        let height = if deep && number == 13 { 2 } else { 1 };
        let tail = if bounded {
            0
        } else {
            *children.last().unwrap()
        };
        bytes[number as usize * 4096..(number as usize + 1) * 4096]
            .copy_from_slice(&page(height, &entries, 0, 0, tail));
    }
    Model {
        bytes,
        keys,
        ordered,
        leaves,
        nodes,
        descending,
    }
}

pub fn subtree_rows(
    number: u32,
    leaves: &[(u32, Vec<usize>)],
    nodes: &[(u32, Vec<u32>, bool)],
) -> Vec<usize> {
    if let Some((_, rows)) = leaves.iter().find(|leaf| leaf.0 == number) {
        return rows.clone();
    }
    let (_, children, _) = nodes.iter().find(|node| node.0 == number).unwrap();
    children
        .iter()
        .flat_map(|child| subtree_rows(*child, leaves, nodes))
        .collect()
}

pub fn verify(model: &Model, candidate: &Ace16DeletePreview, deleted: &[usize]) {
    let before = &model.bytes;
    let after = candidate.candidate_bytes();
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.deleted_rows(), deleted.len());
    assert_eq!(before.len(), after.len());
    assert_eq!(
        u32::from_le_bytes(after[10 * 4096 + 16..10 * 4096 + 20].try_into().unwrap()),
        8 - deleted.len() as u32
    );
    assert_eq!(
        &before[10 * 4096 + 67..10 * 4096 + 71],
        &after[10 * 4096 + 67..10 * 4096 + 71]
    );
    assert_eq!(&before[14 * 4096..15 * 4096], &after[14 * 4096..15 * 4096]);
    let mut allowed = vec![false; before.len()];
    allowed[10 * 4096 + 16..10 * 4096 + 20].fill(true);
    for row in 0..8 {
        let offset = 12 * 4096 + 14 + row * 2;
        let old = u16::from_le_bytes(before[offset..offset + 2].try_into().unwrap());
        let new = u16::from_le_bytes(after[offset..offset + 2].try_into().unwrap());
        assert_eq!(
            new,
            if deleted.contains(&row) {
                old | 0xc000
            } else {
                old
            }
        );
        if deleted.contains(&row) {
            allowed[offset..offset + 2].fill(true);
        }
    }
    for number in model
        .leaves
        .iter()
        .map(|leaf| leaf.0)
        .chain(model.nodes.iter().map(|node| node.0))
    {
        let offset = number as usize * 4096;
        assert_eq!(&before[offset..offset + 2], &after[offset..offset + 2]);
        assert_eq!(
            &before[offset + 4..offset + 24],
            &after[offset + 4..offset + 24]
        );
        assert_eq!(before[offset + 26], after[offset + 26]);
        let old_end = 4096
            - usize::from(u16::from_le_bytes(
                before[offset + 2..offset + 4].try_into().unwrap(),
            ));
        let new_end = 4096
            - usize::from(u16::from_le_bytes(
                after[offset + 2..offset + 4].try_into().unwrap(),
            ));
        assert_eq!(
            &before[offset + old_end.max(new_end)..offset + 4096],
            &after[offset + old_end.max(new_end)..offset + 4096],
            "unused residual bytes on page {number}"
        );
        allowed[offset + 2..offset + 4].fill(true);
        allowed[offset + 24..offset + 26].fill(true);
        allowed[offset + 27..offset + 4096].fill(true);
    }
    for (offset, (&old, &new)) in before.iter().zip(after).enumerate() {
        assert!(allowed[offset] || old == new, "unapproved byte {offset}");
    }
    for (number, children, bounded) in &model.nodes {
        let expected = children
            .iter()
            .take(if *bounded {
                children.len()
            } else {
                children.len() - 1
            })
            .map(|child| {
                let row = subtree_rows(*child, &model.leaves, &model.nodes)
                    .into_iter()
                    .rfind(|row| !deleted.contains(row))
                    .unwrap();
                let mut bytes = entry(model.keys[row], row, model.descending);
                bytes.extend(child.to_be_bytes());
                bytes
            })
            .collect::<Vec<_>>();
        // Before prefix is zero and the existing repacker preserves it when it fits.
        assert_eq!(
            &after[*number as usize * 4096 + 24..*number as usize * 4096 + 26],
            &[0, 0]
        );
        let expected = expected.concat();
        assert_eq!(
            &after[*number as usize * 4096 + 480..*number as usize * 4096 + 480 + expected.len()],
            expected
        );
    }
    let expected = model
        .ordered
        .iter()
        .filter(|row| !deleted.contains(row))
        .map(|row| Value::Int32(*row as i32 + 1))
        .collect::<Vec<_>>();
    for name in if before[10 * 4096 + 47] == 2 {
        vec!["ScoreKey", "ScoreAlias"]
    } else {
        vec!["ScoreKey"]
    } {
        let rows = read_table_index(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows()
                .iter()
                .map(|row| row[0].clone())
                .collect::<Vec<_>>(),
            expected
        );
    }
    check_table(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
}
