use super::*;
use fixture_model::{ENTRY, HEIGHT, Kind, MAPS, Model, NEXT, PAGE, PREFIX, PREVIOUS, ROOTS, TAIL};

const LATE_EOF: usize = 30;

#[derive(Clone)]
struct Child {
    number: usize,
    maximum: Vec<u8>,
}

enum Contents {
    Leaf(Vec<Vec<u8>>),
    Branch(Vec<Child>),
}

struct Node {
    number: usize,
    height: u8,
    tail: bool,
    contents: Contents,
}

impl Node {
    fn entries(&self) -> Vec<Vec<u8>> {
        match &self.contents {
            Contents::Leaf(keys) => keys.clone(),
            Contents::Branch(children) => children[..children.len() - usize::from(self.tail)]
                .iter()
                .map(|child| fixture_model::bound(&child.maximum, child.number))
                .collect(),
        }
    }

    fn maximum(&self) -> Vec<u8> {
        match &self.contents {
            Contents::Leaf(keys) => keys.last().unwrap().clone(),
            Contents::Branch(children) => children.last().unwrap().maximum.clone(),
        }
    }

    fn split_left(&mut self, number: usize) -> Node {
        let contents = match &mut self.contents {
            Contents::Leaf(keys) => {
                let right = keys.split_off(keys.len() / 2);
                Contents::Leaf(std::mem::replace(keys, right))
            }
            Contents::Branch(children) => {
                let right = children.split_off(children.len() / 2);
                Contents::Branch(std::mem::replace(children, right))
            }
        };
        Node {
            number,
            height: self.height,
            tail: self.height > 0,
            contents,
        }
    }
}

pub struct Target {
    pub source: Model,
    pub changes: Vec<(usize, i64)>,
}

pub struct Expected {
    pub bytes: Vec<u8>,
    pub logical_allocations: Vec<Vec<usize>>,
    pub logical_counts: Vec<Vec<usize>>,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

pub fn root(code: u8, kind: Kind, count: usize, extremes: bool) -> Target {
    let source = fixture_model::source(code, kind, vec![count], false);
    let mut changes = vec![(0, kind.maximum())];
    if extremes {
        changes.push((1, i64::MIN));
    }
    Target { source, changes }
}

pub fn late_source(kind: Kind) -> Target {
    let mut target = root(6, kind, 900, false);
    target.source.bytes.resize(LATE_EOF * PAGE, 0);
    for number in target.source.eof..LATE_EOF {
        let offset = root_model::map_byte(&target.source.bytes, 1, number);
        target.source.bytes[offset] &= !(1 << (number % 8));
    }
    target.source.eof = LATE_EOF;
    target
}

pub fn parent(kind: Kind) -> Target {
    Target {
        source: fixture_model::source(6, kind, vec![1; 600], false),
        changes: vec![(0, kind.maximum())],
    }
}

pub fn null_peer(kind: Kind) -> Target {
    Target {
        source: fixture_model::source(6, kind, vec![900, 1], true),
        changes: vec![(0, kind.maximum())],
    }
}

pub fn requests(target: &Target) -> Vec<FixedScalarUpdate> {
    target
        .changes
        .iter()
        .map(|&(row, value)| {
            FixedScalarUpdate::new(
                row,
                target.source.kind.value(target.source.values[row].unwrap()),
                target.source.kind.value(value),
            )
        })
        .collect()
}

pub fn run(target: &Target) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    root_model::capture(&target.source.bytes).preview_ace16_split_fixed_updates(
        "Items",
        "Score",
        &requests(target),
        &mut budget(),
        &|| false,
    )
}

pub fn check(target: &Target) {
    fixture_model::check(&target.source);
}

fn fits(node: &Node) -> bool {
    let entries = node.entries();
    fixture_model::encoded_len(&entries, fixture_model::common(&entries, node.height > 0))
        <= PAGE - ENTRY
}

fn allocate(after: &mut Vec<u8>, allocated: &mut Vec<usize>) -> usize {
    let number = after.len() / PAGE;
    after.resize(after.len() + PAGE, 0);
    allocated.push(number);
    number
}

fn settle(mut nodes: Vec<Node>, after: &mut Vec<u8>, allocated: &mut Vec<usize>) -> Vec<Node> {
    let mut logical = (0..nodes.len()).collect::<Vec<_>>();
    if nodes.len() == 2 {
        // Initial root fragments are passed in allocation order RIGHT, LEFT.
        logical.reverse();
    }
    loop {
        let mut split = false;
        let mut position = 0;
        while position < nodes.len() {
            if !fits(&nodes[position]) {
                let number = allocate(after, allocated);
                let left = nodes[position].split_left(number);
                let logical_position = logical.iter().position(|&index| index == position).unwrap();
                logical.insert(logical_position, nodes.len());
                nodes.push(left);
                split = true;
            }
            position += 1;
        }
        if !split {
            break;
        }
    }
    let mut slots = nodes.into_iter().map(Some).collect::<Vec<_>>();
    logical
        .into_iter()
        .map(|index| slots[index].take().unwrap())
        .collect()
}

fn fragments(mut original: Node, after: &mut Vec<u8>, allocated: &mut Vec<usize>) -> Vec<Node> {
    let right_number = allocate(after, allocated);
    let left_number = allocate(after, allocated);
    let mut left = original.split_left(left_number);
    original.number = right_number;
    original.tail = original.height > 0;
    left.tail = left.height > 0;
    settle(vec![original, left], after, allocated)
}

fn serialized(source: &Model, node: &Node) -> Vec<u8> {
    let entries = node.entries();
    let mut initial = if node.number < source.eof {
        source.bytes[node.number * PAGE..(node.number + 1) * PAGE].to_vec()
    } else {
        fixture_model::blank(node.height, true)
    };
    let seed = root_model::get16(&initial, PREFIX) as usize;
    let best = fixture_model::common(&entries, node.height > 0);
    let mut prefix = seed.min(best);
    if fixture_model::encoded_len(&entries, prefix) > PAGE - ENTRY {
        prefix = best;
    }
    initial[HEIGHT] = node.height;
    let tail = match &node.contents {
        Contents::Branch(children) if node.tail => children.last().unwrap().number,
        _ => 0,
    };
    support::put32(&mut initial, TAIL, tail as u32);
    fixture_model::repack(&initial, &entries, prefix)
}

fn emit(source: &Model, nodes: &[Node], after: &mut [u8]) -> Vec<Child> {
    let mut children = Vec::new();
    for (position, node) in nodes.iter().enumerate() {
        let mut page = serialized(source, node);
        let previous = if position == 0 {
            0
        } else {
            nodes[position - 1].number
        };
        let next = nodes.get(position + 1).map(|node| node.number).unwrap_or(0);
        support::put32(&mut page, PREVIOUS, previous as u32);
        support::put32(&mut page, NEXT, next as u32);
        fixture_model::install(after, node.number, &page);
        children.push(Child {
            number: node.number,
            maximum: node.maximum(),
        });
    }
    children
}

fn root_page(
    source: &Model,
    physical: usize,
    children: Vec<Child>,
    height: u8,
    tail: bool,
    grown: bool,
    after: &mut [u8],
) {
    let number = ROOTS[physical];
    let node = Node {
        number,
        height,
        tail,
        contents: Contents::Branch(children),
    };
    let mut initial = source.bytes[number * PAGE..(number + 1) * PAGE].to_vec();
    initial[..2].copy_from_slice(&[3, 1]);
    initial[HEIGHT] = height;
    support::put32(&mut initial, PREVIOUS, 0);
    support::put32(&mut initial, NEXT, 0);
    let tail_number = match &node.contents {
        Contents::Branch(children) if tail => children.last().unwrap().number,
        _ => 0,
    };
    support::put32(&mut initial, TAIL, tail_number as u32);
    let entries = node.entries();
    let best = fixture_model::common(&entries, true);
    let seed = if grown {
        0
    } else {
        root_model::get16(&initial, PREFIX) as usize
    };
    let mut prefix = seed.min(best);
    if fixture_model::encoded_len(&entries, prefix) > PAGE - ENTRY {
        prefix = best;
    }
    let page = fixture_model::repack(&initial, &entries, prefix);
    fixture_model::install(after, number, &page);
}

pub fn expected(target: &Target) -> Expected {
    let source = &target.source;
    let mut after = source.bytes.clone();
    let mut values = source.values.clone();
    for &(row, value) in &target.changes {
        let offset = fixture_model::field_offset(&source.bytes, row);
        let wire = value.to_le_bytes();
        after[offset..offset + source.kind.width()].copy_from_slice(&wire[..source.kind.width()]);
        values[row] = Some(value);
    }
    let mut logical_allocations = Vec::new();
    let mut logical_counts = Vec::new();
    for physical in 0..2 {
        let mut allocated = Vec::new();
        let keys = fixture_model::ordered(source, &values, physical == 1);
        let nodes = if source.root_leaf {
            let original = Node {
                number: ROOTS[physical],
                height: 0,
                tail: false,
                contents: Contents::Leaf(keys),
            };
            fragments(original, &mut after, &mut allocated)
        } else {
            let mut counts = source.counts.clone();
            if physical == 0 {
                counts.reverse();
            }
            let mut nodes = Vec::new();
            let mut start = 0;
            for (position, count) in counts.into_iter().enumerate() {
                let leaf = Node {
                    number: source.leaves[physical][position],
                    height: 0,
                    tail: false,
                    contents: Contents::Leaf(keys[start..start + count].to_vec()),
                };
                start += count;
                nodes.extend(settle(vec![leaf], &mut after, &mut allocated));
            }
            nodes
        };
        let counts = nodes
            .iter()
            .map(|node| match &node.contents {
                Contents::Leaf(keys) => keys.len(),
                _ => unreachable!(),
            })
            .collect();
        logical_allocations.push(nodes.iter().map(|node| node.number).collect());
        logical_counts.push(counts);
        let children = emit(source, &nodes, &mut after);
        if source.root_leaf {
            root_page(source, physical, children, 1, true, true, &mut after);
        } else {
            let original = Node {
                number: ROOTS[physical],
                height: 1,
                tail: false,
                contents: Contents::Branch(children.clone()),
            };
            if fits(&original) {
                root_page(source, physical, children, 1, false, false, &mut after);
            } else {
                let branches = fragments(original, &mut after, &mut allocated);
                let children = emit(source, &branches, &mut after);
                root_page(source, physical, children, 2, false, true, &mut after);
            }
        }
        for number in allocated {
            let owned = root_model::map_byte(&after, MAPS[physical], number);
            after[owned] |= 1 << (number % 8);
            let global = root_model::map_byte(&after, 1, number);
            assert_ne!(source.bytes[global] & (1 << (number % 8)), 0);
            after[global] &= !(1 << (number % 8));
        }
    }
    Expected {
        bytes: after,
        logical_allocations,
        logical_counts,
    }
}
