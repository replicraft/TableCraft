//! Literal B+tree frames and independent maximal-fit partitioning.
use super::Row;
use crate::fixtures::access::{put16, put32};
use std::ops::Range;

const CAPACITY: usize = 3616;

#[derive(Clone, Copy)]
struct Wire {
    bytes: [u8; 13],
    length: usize,
    key_row: usize,
}

impl Wire {
    fn row(row: Row, descending: bool) -> Self {
        let mut bytes = [0; 13];
        let key = if let Some(value) = row.score {
            bytes[0] = 127;
            bytes[1..5].copy_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
            5
        } else {
            1
        };
        if descending {
            for byte in &mut bytes[..key] {
                *byte = !*byte;
            }
        }
        bytes[key..key + 4].copy_from_slice(&row.reference.to_be_bytes());
        Self {
            bytes,
            length: key + 4,
            key_row: key + 4,
        }
    }

    fn child(self, child: u32) -> Self {
        let mut result = self;
        result.bytes[self.key_row..self.key_row + 4].copy_from_slice(&child.to_be_bytes());
        result.length = self.key_row + 4;
        result
    }
}

struct LevelNode {
    number: u32,
    maximum: Wire,
}

pub(super) struct Tree {
    pub pages: Vec<Vec<u8>>,
    pub root: u32,
    pub leaf_pages: usize,
    pub levels: usize,
}

// Every trial adds one wire and only shortens the shared prefix. Occupied storage
// therefore cannot shrink after a failed trial, so the first failure ends a page.
fn fitting_end(wires: &[Wire], start: usize) -> usize {
    let mut end = start;
    let mut total = 0;
    let mut prefix = usize::MAX;
    while let Some(wire) = wires.get(end) {
        let mut next_prefix = prefix.min(wire.key_row);
        if end != start {
            next_prefix = (0..next_prefix)
                .take_while(|&byte| wires[start].bytes[byte] == wire.bytes[byte])
                .count();
        }
        let next_total = total + wire.length;
        let count = end + 1 - start;
        let used = if next_total <= CAPACITY {
            next_total
        } else {
            next_total - next_prefix * (count - 1)
        };
        if used > CAPACITY {
            break;
        }
        total = next_total;
        prefix = next_prefix;
        end += 1;
    }
    end
}

fn shared_prefix(wires: &[Wire]) -> usize {
    let total = wires.iter().map(|wire| wire.length).sum::<usize>();
    if total <= CAPACITY {
        return 0;
    }
    let Some(first) = wires.first() else {
        return 0;
    };
    let mut prefix = first.key_row;
    for wire in &wires[1..] {
        prefix = prefix.min(wire.key_row);
        prefix = (0..prefix)
            .take_while(|&byte| first.bytes[byte] == wire.bytes[byte])
            .count();
    }
    prefix
}

struct Frame {
    owner: u32,
    previous: u32,
    next: u32,
    tail: Option<u32>,
}

fn page(wires: &[Wire], frame: Frame) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[if frame.tail.is_some() { 3 } else { 4 }, 1]);
    put32(&mut page, 4, frame.owner);
    put32(&mut page, 12, frame.previous);
    put32(&mut page, 16, frame.next);
    put32(&mut page, 20, frame.tail.unwrap_or(0));
    let prefix = shared_prefix(wires);
    put16(&mut page, 24, prefix as u16);
    page[26] = u8::from(frame.tail.is_some());
    let mut used = 0;
    for (position, wire) in wires.iter().enumerate() {
        let skip = if position == 0 { 0 } else { prefix };
        let bytes = &wire.bytes[skip..wire.length];
        assert!(used + bytes.len() <= CAPACITY);
        page[480 + used..480 + used + bytes.len()].copy_from_slice(bytes);
        used += bytes.len();
        page[27 + used / 8] |= 1 << (used % 8);
    }
    put16(&mut page, 2, (CAPACITY - used) as u16);
    page
}

fn siblings(first: u32, position: usize, count: usize) -> (u32, u32) {
    (
        if position == 0 {
            0
        } else {
            first + position as u32 - 1
        },
        if position + 1 == count {
            0
        } else {
            first + position as u32 + 1
        },
    )
}

pub(super) fn build(rows: &[Row], first: u32, owner: u32, descending: bool) -> Tree {
    let mut wires = rows
        .iter()
        .map(|row| Wire::row(*row, descending))
        .collect::<Vec<_>>();
    wires.sort_unstable_by(|a, b| a.bytes[..a.length].cmp(&b.bytes[..b.length]));
    let mut leaves = Vec::new();
    let mut start = 0;
    while start < wires.len() {
        let end = fitting_end(&wires, start);
        assert!(end > start);
        leaves.push(start..end);
        start = end;
    }
    if leaves.is_empty() {
        leaves.push(0..0);
    }
    let mut pages = Vec::new();
    let mut level = Vec::new();
    for (position, range) in leaves.iter().enumerate() {
        let number = first + position as u32;
        let (previous, next) = siblings(first, position, leaves.len());
        pages.push(page(
            &wires[range.start..range.end],
            Frame {
                owner,
                previous,
                next,
                tail: None,
            },
        ));
        level.push(LevelNode {
            number,
            maximum: range.end.checked_sub(1).map_or(
                Wire {
                    bytes: [0; 13],
                    length: 0,
                    key_row: 0,
                },
                |last| wires[last],
            ),
        });
    }
    let leaf_pages = pages.len();
    let mut levels = 1;
    while level.len() > 1 {
        // A node omits its final child's fence. The fitting wire immediately
        // before each tail determines the largest contiguous child group.
        let fences = level
            .iter()
            .map(|child| child.maximum.child(child.number))
            .collect::<Vec<_>>();
        let mut groups: Vec<Range<usize>> = Vec::new();
        let mut start = 0;
        while start < level.len() {
            let entries_end = fitting_end(&fences, start).min(level.len() - 1);
            let end = entries_end + 1;
            assert!(end > start);
            groups.push(start..end);
            start = end;
        }
        if groups.last().is_some_and(|range| range.len() == 1) {
            let last = groups.len() - 1;
            assert!(last != 0 && groups[last - 1].len() > 2);
            groups[last - 1].end -= 1;
            groups[last].start -= 1;
        }
        let first_parent = first + pages.len() as u32;
        let mut parents = Vec::new();
        for (position, range) in groups.iter().enumerate() {
            assert!(range.len() >= 2);
            let tail = &level[range.end - 1];
            let (previous, next) = siblings(first_parent, position, groups.len());
            pages.push(page(
                &fences[range.start..range.end - 1],
                Frame {
                    owner,
                    previous,
                    next,
                    tail: Some(tail.number),
                },
            ));
            parents.push(LevelNode {
                number: first_parent + position as u32,
                maximum: tail.maximum,
            });
        }
        level = parents;
        levels += 1;
    }
    assert_eq!(level[0].number, first + pages.len() as u32 - 1);
    Tree {
        pages,
        root: level[0].number,
        leaf_pages,
        levels,
    }
}
