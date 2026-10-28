//! Independent recursive INSERT rows, tree frames and complete images.
use super::super::model as base;
use crate::RowInsert;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as literal;

pub(super) const PAGE: usize = 4096;

pub(super) struct Row {
    pub id: i32,
    pub binary: Option<Vec<u8>>,
}

impl Row {
    pub fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.id),
            self.binary
                .as_ref()
                .map_or(Value::Null, |bytes| Value::Binary(bytes.clone())),
        ]
    }

    pub fn request(&self) -> RowInsert {
        RowInsert::new(self.values())
    }
}

fn put(bytes: &mut [u8], at: usize, value: u32, width: usize) {
    bytes[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]);
}

fn install(bytes: &mut [u8], number: u32, page: &[u8]) {
    assert_eq!(page.len(), PAGE);
    let start = number as usize * PAGE;
    bytes[start..start + PAGE].copy_from_slice(page);
}

fn inline(members: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for number in members {
        assert!(number < 512);
        bytes[5 + number as usize / 8] |= 1 << (number % 8);
    }
    bytes
}

fn record(id: i32, binary: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend(id.to_le_bytes());
    if let Some(payload) = binary {
        bytes.extend_from_slice(payload);
    }
    let end = bytes.len() as u16;
    bytes.extend(end.to_le_bytes());
    bytes.extend([6, 0, 1, 0, if binary.is_some() { 3 } else { 1 }]);
    bytes
}

fn key(id: i32, reference: u32) -> [u8; 9] {
    let mut bytes = [0; 9];
    bytes[0] = 127;
    bytes[1..5].copy_from_slice(&((id as u32) ^ 0x80000000).to_be_bytes());
    bytes[5..].copy_from_slice(&reference.to_be_bytes());
    bytes
}

fn common(entries: &[[u8; 9]]) -> usize {
    let mut width = 9;
    for entry in &entries[1..] {
        width = entries[0][..width]
            .iter()
            .zip(entry)
            .take_while(|(left, right)| left == right)
            .count();
    }
    width
}

fn prefix(entries: &[[u8; 9]], seed: usize, child: bool) -> (usize, usize) {
    assert!(!entries.is_empty());
    let best = common(entries);
    let mut prefix = seed.min(best);
    let full = entries.len() * (9 + if child { 4 } else { 0 });
    let saved = entries.len() - 1;
    if full - saved * prefix > 3616 {
        prefix = best;
    }
    (prefix, full - saved * prefix)
}

fn leaf(
    original: Option<&[u8]>,
    entries: &[[u8; 9]],
    seed: usize,
    previous: u32,
    next: u32,
) -> Vec<u8> {
    let (prefix, active) = prefix(entries, seed, false);
    assert!(active <= 3616);
    let mut page = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    page[..2].copy_from_slice(&[4, 1]);
    put(&mut page, 2, (3616 - active) as u32, 2);
    put(&mut page, 4, 10, 4);
    put(&mut page, 12, previous, 4);
    put(&mut page, 16, next, 4);
    put(&mut page, 20, 0, 4);
    put(&mut page, 24, prefix as u32, 2);
    page[26] = 0;
    page[27..480].fill(0);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let wire = &entry[if position == 0 { 0 } else { prefix }..];
        page[480 + end..480 + end + wire.len()].copy_from_slice(wire);
        end += wire.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    if let Some(original) = original {
        assert_eq!(&page[480 + active..], &original[480 + active..]);
    } else {
        assert!(page[480 + active..].iter().all(|byte| *byte == 0));
    }
    page
}

fn bitmap(members: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..4].copy_from_slice(&[5, 1, 0, 0]);
    for number in members {
        assert!(number < 32736);
        page[4 + number as usize / 8] |= 1 << (number % 8);
    }
    page
}

fn indirect(number: u32) -> Vec<u8> {
    let mut row = vec![0; 69];
    row[0] = 1;
    put(&mut row, 1, number, 4);
    row
}

#[derive(Clone)]
pub(super) struct Node {
    pub number: u32,
    pub height: u8,
    pub entries: Vec<[u8; 9]>,
    pub children: Vec<usize>,
    pub tail: bool,
    pub seed: usize,
    original: bool,
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<(i32, Option<Vec<u8>>)>,
    pub nodes: Vec<Node>,
    pub root: usize,
    pub data: Vec<u32>,
    pub hints: Vec<u32>,
    pub aliases: bool,
    pub external_maps: bool,
    owned_bitmap: u32,
    global_bitmap: u32,
}

pub(super) struct Expected {
    pub bytes: Vec<u8>,
    pub nodes: Vec<Node>,
    pub references: Vec<u32>,
    pub fresh: Vec<u32>,
    pub bitmaps: Vec<u32>,
    pub levels: Vec<Vec<u32>>,
    data: Vec<u32>,
    hints: Vec<u32>,
}

#[derive(Clone, Copy)]
pub(super) enum Shape {
    PrefixLoss,
    TwoParents,
    ThreeOffenders,
    HeightThree,
    RootLeaf,
}

pub(super) enum Maps {
    Inline,
    At600,
    RebaseThenPromote,
}

fn maximum(nodes: &[Node], index: usize) -> [u8; 9] {
    let node = &nodes[index];
    if node.height == 0 {
        *node.entries.last().unwrap()
    } else {
        maximum(nodes, *node.children.last().unwrap())
    }
}

fn bounds(nodes: &[Node], index: usize) -> Vec<[u8; 9]> {
    let node = &nodes[index];
    if node.height == 0 {
        node.entries.clone()
    } else {
        node.children[..node.children.len() - usize::from(node.tail)]
            .iter()
            .map(|child| maximum(nodes, *child))
            .collect()
    }
}

fn measurement(nodes: &[Node], index: usize) -> (usize, usize) {
    let entries = bounds(nodes, index);
    if entries.is_empty() {
        (0, 0)
    } else {
        prefix(&entries, nodes[index].seed, nodes[index].height != 0)
    }
}

fn render(
    nodes: &[Node],
    index: usize,
    original: Option<&[u8]>,
    previous: u32,
    next: u32,
) -> Vec<u8> {
    let node = &nodes[index];
    if node.height == 0 {
        return leaf(original, &node.entries, node.seed, previous, next);
    }
    let entries = bounds(nodes, index);
    let (prefix, active) = measurement(nodes, index);
    assert!(active <= 3616);
    let mut page = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    page[..2].copy_from_slice(&[3, 1]);
    put(&mut page, 2, (3616 - active) as u32, 2);
    put(&mut page, 4, 10, 4);
    put(&mut page, 12, previous, 4);
    put(&mut page, 16, next, 4);
    put(
        &mut page,
        20,
        if node.tail {
            nodes[*node.children.last().unwrap()].number
        } else {
            0
        },
        4,
    );
    put(&mut page, 24, prefix as u32, 2);
    page[26] = node.height;
    page[27..480].fill(0);
    let mut end = 0;
    for (position, (entry, child)) in entries.iter().zip(&node.children).enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        page[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        page[480 + end..484 + end].copy_from_slice(&nodes[*child].number.to_be_bytes());
        end += 4;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    page
}

fn centered(center: i32) -> Vec<i32> {
    (center - 226..center)
        .chain(center + 1..=center + 225)
        .collect()
}

fn internal(
    nodes: &mut Vec<Node>,
    number: u32,
    children: Vec<usize>,
    tail: bool,
    seed: usize,
) -> usize {
    let height = nodes[children[0]].height + 1;
    assert!(
        children
            .iter()
            .all(|child| nodes[*child].height + 1 == height)
    );
    let index = nodes.len();
    nodes.push(Node {
        number,
        height,
        entries: Vec::new(),
        children,
        tail,
        seed,
        original: true,
    });
    index
}

fn collect_levels(nodes: &[Node], root: usize) -> Vec<Vec<usize>> {
    fn visit(nodes: &[Node], index: usize, levels: &mut [Vec<usize>]) {
        levels[nodes[index].height as usize].push(index);
        for child in &nodes[index].children {
            visit(nodes, *child, levels);
        }
    }
    let mut levels = vec![Vec::new(); nodes[root].height as usize + 1];
    visit(nodes, root, &mut levels);
    levels
}

pub(super) fn source(code: u8, shape: Shape, maps: Maps, aliases: bool) -> Case {
    let groups = match shape {
        Shape::PrefixLoss => (0..361).map(|id| vec![id]).collect::<Vec<_>>(),
        Shape::TwoParents => (0..360)
            .map(|id| vec![id])
            .chain([
                (1_000_000..1_000_450).chain([1_000_000_000]).collect(),
                vec![i32::MAX],
            ])
            .collect(),
        Shape::ThreeOffenders => vec![centered(0), centered(1 << 24), centered(2 << 24)],
        Shape::HeightThree => vec![vec![0, 2], vec![10, 12], vec![20, 22]],
        Shape::RootLeaf => vec![vec![1, 2, 3]],
    };
    let ids = groups.iter().flatten().copied().collect::<Vec<_>>();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    let data_start = 64.max(40 + groups.len() as u32);
    let data = (0..ids.len().div_ceil(255))
        .map(|position| data_start + position as u32)
        .collect::<Vec<_>>();
    let (eof, owned_bitmap, global_bitmap) = match maps {
        Maps::Inline => (128.max(data.last().unwrap() + 9), 0, 0),
        Maps::At600 => (600, 597, 599),
        Maps::RebaseThenPromote => (518, 515, 517),
    };
    let mut before = base::source(code, aliases, false, None);
    before.resize(eof as usize * PAGE, 0);
    put(&mut before, 10 * PAGE + 16, ids.len() as u32, 4);
    put(&mut before, 10 * PAGE + 67, ids.len() as u32, 4);
    for (number, chunk) in data.iter().zip(ids.chunks(255)) {
        let rows = chunk
            .iter()
            .map(|id| (0, record(*id, None)))
            .collect::<Vec<_>>();
        let mut page = literal::data_page(10, &rows);
        put(&mut page, 2, (4082 - rows.len() * 15) as u32, 2);
        install(&mut before, *number, &page);
    }
    let mut nodes = Vec::new();
    let mut position = 0;
    for (index, group) in groups.iter().enumerate() {
        let entries = group
            .iter()
            .map(|id| {
                let reference = data[position / 255] << 8 | (position % 255) as u32;
                position += 1;
                key(*id, reference)
            })
            .collect::<Vec<_>>();
        nodes.push(Node {
            number: if matches!(shape, Shape::RootLeaf) {
                13
            } else {
                32 + index as u32
            },
            height: 0,
            seed: if matches!(shape, Shape::RootLeaf) {
                0
            } else {
                common(&entries)
            },
            entries,
            children: Vec::new(),
            tail: false,
            original: true,
        });
    }
    let root = match shape {
        Shape::PrefixLoss => internal(&mut nodes, 13, (0..361).collect(), false, 3),
        Shape::TwoParents => {
            let left = internal(&mut nodes, 20, (0..361).collect(), true, 3);
            let right = internal(&mut nodes, 21, vec![361], true, 0);
            internal(&mut nodes, 13, vec![left, right], false, 0)
        }
        Shape::ThreeOffenders => {
            let left = internal(&mut nodes, 20, vec![0, 1], true, 0);
            let right = internal(&mut nodes, 21, vec![2], true, 0);
            internal(&mut nodes, 13, vec![left, right], false, 0)
        }
        Shape::HeightThree => {
            let left = internal(&mut nodes, 20, vec![0, 1], true, 0);
            let right = internal(&mut nodes, 21, vec![2], true, 0);
            let middle = internal(&mut nodes, 22, vec![left, right], false, 0);
            internal(&mut nodes, 13, vec![middle], false, 0)
        }
        Shape::RootLeaf => 0,
    };
    for level in collect_levels(&nodes, root) {
        for (position, index) in level.iter().enumerate() {
            let mut residue = vec![0; PAGE];
            residue[480..].fill(0x90 + (nodes[*index].number % 43) as u8);
            let previous = position
                .checked_sub(1)
                .map_or(0, |i| nodes[level[i]].number);
            let next = level.get(position + 1).map_or(0, |i| nodes[*i].number);
            install(
                &mut before,
                nodes[*index].number,
                &render(&nodes, *index, Some(&residue), previous, next),
            );
        }
    }
    install(
        &mut before,
        14,
        &literal::data_page(0, &[(0, inline(nodes.iter().map(|node| node.number)))]),
    );
    if owned_bitmap == 0 {
        install(
            &mut before,
            11,
            &literal::data_page(
                0,
                &[
                    (0, inline(data.iter().copied())),
                    (0, inline(data.iter().copied())),
                ],
            ),
        );
        install(
            &mut before,
            1,
            &literal::data_page(1, &[(0, inline(eof..512))]),
        );
    } else {
        install(&mut before, owned_bitmap, &bitmap(data.iter().copied()));
        install(&mut before, owned_bitmap + 1, &bitmap([7]));
        install(&mut before, global_bitmap, &bitmap(eof..32736));
        install(
            &mut before,
            11,
            &literal::data_page(
                0,
                &[
                    (0, indirect(owned_bitmap)),
                    (0, inline(data.iter().copied())),
                ],
            ),
        );
        install(
            &mut before,
            1,
            &literal::data_page(1, &[(0, indirect(global_bitmap))]),
        );
    }
    Case {
        before,
        rows: ids.into_iter().map(|id| (id, None)).collect(),
        nodes,
        root,
        hints: data.clone(),
        data,
        aliases,
        external_maps: owned_bitmap != 0,
        owned_bitmap,
        global_bitmap,
    }
}
fn append_data(bytes: &mut Vec<u8>, hints: &mut Vec<u32>, owned: &mut Vec<u32>, row: &Row) -> u32 {
    let payload = record(row.id, row.binary.as_deref());
    let mut number = *hints.last().unwrap();
    let mut base = number as usize * PAGE;
    let mut count = usize::from(u16::from_le_bytes(
        bytes[base + 12..base + 14].try_into().unwrap(),
    ));
    let free = usize::from(u16::from_le_bytes(
        bytes[base + 2..base + 4].try_into().unwrap(),
    ));
    if count == 255 || payload.len() + 2 > free {
        hints.pop();
        number = (bytes.len() / PAGE) as u32;
        let mut fresh = vec![0; PAGE];
        fresh[..2].copy_from_slice(&[1, 1]);
        put(&mut fresh, 2, 4082, 2);
        put(&mut fresh, 4, 10, 4);
        bytes.extend(fresh);
        owned.push(number);
        hints.push(number);
        count = 0;
        base = number as usize * PAGE;
    }
    let end = if count == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[base + 12 + 2 * count..base + 14 + 2 * count]
                .try_into()
                .unwrap(),
        )) & 0x1fff
    };
    let start = end - payload.len();
    bytes[base + start..base + end].copy_from_slice(&payload);
    put(bytes, base + 14 + 2 * count, start as u32, 2);
    put(bytes, base + 12, (count + 1) as u32, 2);
    put(bytes, base + 2, (start - 16 - 2 * count) as u32, 2);
    (number << 8) | count as u32
}

fn route(nodes: &[Node], index: usize, entry: &[u8; 9]) -> usize {
    if nodes[index].height == 0 {
        return index;
    }
    let children = &nodes[index].children;
    let child = children[..children.len() - 1]
        .iter()
        .find(|child| *entry <= maximum(nodes, **child))
        .unwrap_or_else(|| children.last().unwrap());
    route(nodes, *child, entry)
}

fn divide(nodes: &mut Vec<Node>, index: usize, creation: &mut Vec<usize>) -> usize {
    let mut left = nodes[index].clone();
    left.number = 0;
    left.original = false;
    left.seed = 0;
    if left.height == 0 {
        let half = left.entries.len() / 2;
        nodes[index].entries = left.entries.split_off(half);
    } else {
        let half = left.children.len() / 2;
        nodes[index].children = left.children.split_off(half);
        left.tail = true;
    }
    let added = nodes.len();
    nodes.push(left);
    creation.push(added);
    added
}

fn finish_local(
    nodes: &mut Vec<Node>,
    right: usize,
    left: usize,
    creation: &mut Vec<usize>,
) -> Vec<usize> {
    let mut work = vec![right, left];
    let mut logical = vec![left, right];
    loop {
        let mut changed = false;
        let mut cursor = 0;
        while cursor < work.len() {
            let index = work[cursor];
            if measurement(nodes, index).1 > 3616 {
                let added = divide(nodes, index, creation);
                work.push(added);
                let position = logical.iter().position(|item| *item == index).unwrap();
                logical.insert(position, added);
                changed = true;
            }
            cursor += 1;
        }
        if !changed {
            return logical;
        }
    }
}

fn normalize(
    nodes: &mut Vec<Node>,
    index: usize,
    root: usize,
    creation: &mut Vec<usize>,
) -> Vec<usize> {
    let original_children = nodes[index].children.clone();
    let mut children = Vec::new();
    for child in original_children {
        children.extend(normalize(nodes, child, root, creation));
    }
    nodes[index].children = children;
    if measurement(nodes, index).1 <= 3616 {
        return vec![index];
    }
    if index != root {
        let left = divide(nodes, index, creation);
        return finish_local(nodes, index, left, creation);
    }
    let old_height = nodes[index].height;
    assert!(old_height < 15);
    let mut right = nodes[index].clone();
    right.original = false;
    right.number = 0;
    right.seed = 0;
    right.tail = old_height != 0;
    let nested = nodes.len();
    nodes.push(right);
    creation.push(nested);
    let left = divide(nodes, nested, creation);
    let final_children = finish_local(nodes, nested, left, creation);
    nodes[index].height = old_height + 1;
    nodes[index].seed = 0;
    nodes[index].tail = old_height == 0;
    nodes[index].entries.clear();
    nodes[index].children = final_children;
    assert!(measurement(nodes, index).1 <= 3616);
    vec![index]
}

fn inline_at(base: u32, members: &[u32]) -> Vec<u8> {
    let mut row = vec![0; 69];
    put(&mut row, 1, base, 4);
    for number in members {
        let relative = number.checked_sub(base).unwrap();
        assert!(relative < 512);
        row[5 + relative as usize / 8] |= 1 << (relative % 8);
    }
    row
}

pub(super) fn expected(case: &Case, rows: &[Row]) -> Expected {
    let mut bytes = case.before.clone();
    let mut data = case.data.clone();
    let mut hints = case.hints.clone();
    let references = rows
        .iter()
        .map(|row| append_data(&mut bytes, &mut hints, &mut data, row))
        .collect::<Vec<_>>();
    let mut nodes = case.nodes.clone();
    for (row, reference) in rows.iter().zip(&references) {
        let wire = key(row.id, *reference);
        let leaf = route(&case.nodes, case.root, &wire);
        nodes[leaf].entries.push(wire);
    }
    for node in &mut nodes {
        node.entries.sort_unstable();
        assert!(
            node.entries
                .windows(2)
                .all(|pair| pair[0][..5] != pair[1][..5])
        );
    }
    let mut creation = Vec::new();
    assert_eq!(
        normalize(&mut nodes, case.root, case.root, &mut creation),
        [case.root]
    );
    let mut members = case
        .nodes
        .iter()
        .map(|node| node.number)
        .collect::<Vec<_>>();
    let mut base = 0;
    let mut bitmap_page = None;
    let mut fresh = Vec::new();
    let mut bitmaps = Vec::new();
    for index in creation {
        let number = (bytes.len() / PAGE) as u32;
        bytes.extend(vec![0; PAGE]);
        nodes[index].number = number;
        fresh.push(number);
        members.push(number);
        if bitmap_page.is_none() && !(base..base + 512).contains(&number) {
            let minimum = *members.iter().min().unwrap();
            let maximum = *members.iter().max().unwrap();
            let aligned = minimum / 8 * 8;
            if maximum - aligned + 1 < 512 {
                base = aligned;
            } else {
                assert!(case.external_maps);
                let metadata = (bytes.len() / PAGE) as u32;
                bytes.extend(vec![0; PAGE]);
                bitmap_page = Some(metadata);
                bitmaps.push(metadata);
            }
        }
    }
    let indexes = collect_levels(&nodes, case.root);
    for level in &indexes {
        for (position, index) in level.iter().enumerate() {
            let node = &nodes[*index];
            let original = node.original.then(|| {
                &case.before[node.number as usize * PAGE..(node.number as usize + 1) * PAGE]
            });
            let previous = position
                .checked_sub(1)
                .map_or(0, |i| nodes[level[i]].number);
            let next = level.get(position + 1).map_or(0, |i| nodes[*i].number);
            install(
                &mut bytes,
                node.number,
                &render(&nodes, *index, original, previous, next),
            );
        }
    }
    if case.external_maps {
        install(&mut bytes, case.owned_bitmap, &bitmap(data.iter().copied()));
        let eof = (bytes.len() / PAGE) as u32;
        install(&mut bytes, case.global_bitmap, &bitmap(eof..32736));
        install(
            &mut bytes,
            11,
            &literal::data_page(
                0,
                &[
                    (0, indirect(case.owned_bitmap)),
                    (0, inline(hints.iter().copied())),
                ],
            ),
        );
    } else {
        install(
            &mut bytes,
            11,
            &literal::data_page(
                0,
                &[
                    (0, inline(data.iter().copied())),
                    (0, inline(hints.iter().copied())),
                ],
            ),
        );
        let eof = (bytes.len() / PAGE) as u32;
        install(
            &mut bytes,
            1,
            &literal::data_page(1, &[(0, inline(eof..512))]),
        );
    }
    let map = if let Some(number) = bitmap_page {
        install(&mut bytes, number, &bitmap(members.iter().copied()));
        indirect(number)
    } else {
        inline_at(base, &members)
    };
    install(&mut bytes, 14, &literal::data_page(0, &[(0, map)]));
    put(
        &mut bytes,
        10 * PAGE + 16,
        (case.rows.len() + rows.len()) as u32,
        4,
    );
    let history = u32::from_le_bytes(
        case.before[10 * PAGE + 67..10 * PAGE + 71]
            .try_into()
            .unwrap(),
    );
    put(&mut bytes, 10 * PAGE + 67, history + rows.len() as u32, 4);
    let levels = indexes
        .into_iter()
        .map(|level| level.into_iter().map(|index| nodes[index].number).collect())
        .collect();
    Expected {
        bytes,
        nodes,
        references,
        fresh,
        bitmaps,
        levels,
        data,
        hints,
    }
}

pub(super) fn next(case: &Case, rows: &[Row], mut expected: Expected) -> Case {
    assert!(
        !case.external_maps,
        "This next-source fixture uses represented inline maps only"
    );
    let mut all_rows = case.rows.clone();
    all_rows.extend(rows.iter().map(|row| (row.id, row.binary.clone())));
    for node in &mut expected.nodes {
        node.original = true;
        node.seed = usize::from(u16::from_le_bytes(
            expected.bytes[node.number as usize * PAGE + 24..node.number as usize * PAGE + 26]
                .try_into()
                .unwrap(),
        ));
    }
    Case {
        before: expected.bytes,
        rows: all_rows,
        nodes: expected.nodes,
        root: case.root,
        data: expected.data,
        hints: expected.hints,
        aliases: case.aliases,
        external_maps: false,
        owned_bitmap: 0,
        global_bitmap: 0,
    }
}
fn decode_page(bytes: &[u8], number: u32) -> (Vec<[u8; 9]>, Vec<u32>) {
    let page = &bytes[number as usize * PAGE..(number as usize + 1) * PAGE];
    let prefix = usize::from(u16::from_le_bytes(page[24..26].try_into().unwrap()));
    let active = 3616 - usize::from(u16::from_le_bytes(page[2..4].try_into().unwrap()));
    let mut entries = Vec::<[u8; 9]>::new();
    let mut children = Vec::new();
    let mut start = 0;
    for end in 1..=active {
        if page[27 + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let child_width = if page[26] == 0 { 0 } else { 4 };
        let suffix = &page[480 + start..480 + end - child_width];
        let mut entry = Vec::new();
        if !entries.is_empty() {
            entry.extend_from_slice(&entries[0][..prefix]);
        }
        entry.extend_from_slice(suffix);
        entries.push(entry.try_into().unwrap());
        if child_width != 0 {
            children.push(u32::from_be_bytes(
                page[480 + end - 4..480 + end].try_into().unwrap(),
            ));
        }
        start = end;
    }
    assert_eq!(start, active);
    (entries, children)
}

/// Read the actual old literal H1 source, without changing any original byte.
pub(super) fn height_one_source(before: &[u8], aliases: bool) -> Case {
    let map_begin = 11 * PAGE + 4027;
    assert_eq!(&before[map_begin..map_begin + 5], &[0; 5]);
    let data = (0..512)
        .filter(|number| before[map_begin + 5 + number / 8] & (1 << (number % 8)) != 0)
        .map(|number| number as u32)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut wires = Vec::new();
    for number in &data {
        let page = &before[*number as usize * PAGE..(*number as usize + 1) * PAGE];
        let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
        let mut end = PAGE;
        for slot in 0..count {
            let start = usize::from(u16::from_le_bytes(
                page[14 + 2 * slot..16 + 2 * slot].try_into().unwrap(),
            ));
            assert!(start < end && start & !0x1fff == 0);
            let raw = &page[start..end];
            let id = i32::from_le_bytes(raw[2..6].try_into().unwrap());
            assert_eq!(raw, record(id, None));
            rows.push((id, None));
            wires.push(key(id, *number << 8 | slot as u32));
            end = start;
        }
    }
    let (entries, mut numbers) = decode_page(before, 13);
    let root_page = &before[13 * PAGE..14 * PAGE];
    assert_eq!(root_page[26], 1);
    let tail = u32::from_le_bytes(root_page[20..24].try_into().unwrap());
    if tail != 0 {
        numbers.push(tail);
    }
    assert_eq!(entries.len(), numbers.len() - usize::from(tail != 0));
    let mut nodes = Vec::new();
    let mut leaf_wires = Vec::new();
    for number in numbers {
        let (entries, children) = decode_page(before, number);
        assert!(children.is_empty());
        leaf_wires.extend(entries.iter().copied());
        let page = &before[number as usize * PAGE..(number as usize + 1) * PAGE];
        assert_eq!(page[26], 0);
        nodes.push(Node {
            number,
            height: 0,
            entries,
            children: Vec::new(),
            tail: false,
            seed: usize::from(u16::from_le_bytes(page[24..26].try_into().unwrap())),
            original: true,
        });
    }
    wires.sort_unstable();
    assert_eq!(wires, leaf_wires);
    let count = nodes.len();
    let root = internal(
        &mut nodes,
        13,
        (0..count).collect(),
        tail != 0,
        usize::from(u16::from_le_bytes(root_page[24..26].try_into().unwrap())),
    );
    let hints_begin = 11 * PAGE + 3958;
    assert_eq!(&before[hints_begin..hints_begin + 5], &[0; 5]);
    let hints = (0..512)
        .filter(|number| before[hints_begin + 5 + number / 8] & (1 << (number % 8)) != 0)
        .map(|number| number as u32)
        .collect();
    Case {
        before: before.to_vec(),
        rows,
        nodes,
        root,
        data,
        hints,
        aliases,
        external_maps: false,
        owned_bitmap: 0,
        global_bitmap: 0,
    }
}
