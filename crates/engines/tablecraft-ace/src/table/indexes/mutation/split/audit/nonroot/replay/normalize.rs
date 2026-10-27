//! Recompute stable page passes from keys and origin seeds, never builder events.
use super::super::super::super::super::retained::RetainedProfile;
use super::{
    Branch, Candidate, Child, Payload, Progress, Replay, bounds, framing, leaf_entries, push,
};
use crate::{
    EditError, InspectError, format::reader::Reader, limits::MAX_INDEX_DEPTH,
    table::indexes::header::IndexHeader,
};
use std::io::Cursor;

pub(super) fn finish<P: RetainedProfile, const N: usize>(
    replay: &mut Replay<'_, '_, P, N>,
    mut candidate: Candidate<'_>,
    root: bool,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<Vec<Child>, EditError> {
    if fits(replay, &candidate, actual)? {
        let result = certify(replay, &candidate, actual)?;
        let mut children = Vec::new();
        push(&mut children, result, actual)?;
        return Ok(children);
    }
    if !root {
        let (left, right) = partition(replay, &candidate, actual, progress)?;
        return run(replay, right, left, actual, progress);
    }
    let number = candidate.number;
    let source = candidate.source;
    let leaf_root = matches!(candidate.contents, Payload::Leaf(_));
    let height = candidate
        .height
        .checked_add(1)
        .filter(|height| usize::from(*height) < MAX_INDEX_DEPTH)
        .ok_or(EditError::Preservation)?;
    candidate.number = replay.allocate(actual, progress)?;
    candidate.source = framing::Source::Fresh;
    if let Payload::Branch { tail, .. } = &mut candidate.contents {
        *tail = true;
    }
    let (left, right) = partition(replay, &candidate, actual, progress)?;
    let children = run(replay, right, left, actual, progress)?;
    let framing::Source::Existing(original) = source else {
        return Err(EditError::Preservation);
    };
    let root = Candidate {
        number,
        height,
        contents: Payload::Branch {
            children,
            tail: leaf_root,
        },
        source: framing::Source::GrownRoot(original),
    };
    if !fits(replay, &root, actual)? {
        return Err(EditError::Preservation);
    }
    let child = certify(replay, &root, actual)?;
    let mut result = Vec::new();
    push(&mut result, child, actual)?;
    Ok(result)
}

fn run<P: RetainedProfile, const N: usize>(
    replay: &mut Replay<'_, '_, P, N>,
    right: Candidate<'_>,
    left: Candidate<'_>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<Vec<Child>, EditError> {
    let mut pages = Vec::new();
    push(&mut pages, right, actual)?;
    push(&mut pages, left, actual)?;
    let mut order = Vec::new();
    push(&mut order, 1_usize, actual)?;
    push(&mut order, 0_usize, actual)?;
    loop {
        actual.tick()?;
        let mut divided = false;
        let mut position = 0;
        while position < pages.len() {
            let current = position;
            position += 1;
            if fits(replay, &pages[current], actual)? {
                continue;
            }
            let (left, right) = partition(replay, &pages[current], actual, progress)?;
            pages[current] = right;
            let left_index = pages.len();
            push(&mut pages, left, actual)?;
            actual.work(order.len())?;
            let before = order
                .iter()
                .position(|&index| index == current)
                .ok_or(EditError::Preservation)?;
            actual
                .budget
                .charge_allocation(1, size_of::<usize>() as u64)
                .map_err(InspectError::from)?;
            order.try_reserve(1).map_err(|_| InspectError::Allocation)?;
            order.insert(before, left_index);
            divided = true;
        }
        if !divided {
            break;
        }
    }
    let mut children = Vec::new();
    for index in order {
        let child = certify(replay, &pages[index], actual)?;
        push(&mut children, child, actual)?;
    }
    Ok(children)
}

fn fits<P: RetainedProfile, const N: usize>(
    replay: &Replay<'_, '_, P, N>,
    candidate: &Candidate<'_>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    let entries = match &candidate.contents {
        Payload::Leaf(range) => leaf_entries(
            replay
                .operation
                .keys
                .get(range.clone())
                .ok_or(EditError::Preservation)?,
            actual.budget,
            actual.cancelled,
        )?,
        Payload::Branch { children, tail } => {
            bounds(replay.operation.keys, children, *tail, actual)?
        }
    };
    let seed = match candidate.source {
        framing::Source::Existing(original) => {
            usize::from(IndexHeader::read(original)?.prefix.get())
        }
        framing::Source::Fresh | framing::Source::GrownRoot(_) => 0,
    };
    framing::fits::<P, N>(&entries, seed, replay.operation.profile, actual)
}

fn partition<'a, P: RetainedProfile, const N: usize>(
    replay: &mut Replay<'_, '_, P, N>,
    candidate: &Candidate<'a>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<(Candidate<'a>, Candidate<'a>), EditError> {
    let (left_contents, right_contents) = match &candidate.contents {
        Payload::Leaf(range) => {
            let count = range
                .end
                .checked_sub(range.start)
                .ok_or(EditError::Preservation)?;
            if count < 2 {
                return Err(EditError::Preservation);
            }
            let middle = range.start + count / 2;
            (
                Payload::Leaf(range.start..middle),
                Payload::Leaf(middle..range.end),
            )
        }
        Payload::Branch { children, tail } => {
            if children.len() < 2 {
                return Err(EditError::Preservation);
            }
            let middle = children.len() / 2;
            let mut left = Vec::new();
            let mut right = Vec::new();
            for child in &children[..middle] {
                actual.tick()?;
                push(&mut left, *child, actual)?;
            }
            for child in &children[middle..] {
                actual.tick()?;
                push(&mut right, *child, actual)?;
            }
            (
                Payload::Branch {
                    children: left,
                    tail: true,
                },
                Payload::Branch {
                    children: right,
                    tail: *tail,
                },
            )
        }
    };
    let left = Candidate {
        number: replay.allocate(actual, progress)?,
        height: candidate.height,
        contents: left_contents,
        source: framing::Source::Fresh,
    };
    let right = Candidate {
        number: candidate.number,
        height: candidate.height,
        contents: right_contents,
        source: candidate.source,
    };
    Ok((left, right))
}

fn certify<P: RetainedProfile, const N: usize>(
    replay: &mut Replay<'_, '_, P, N>,
    candidate: &Candidate<'_>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Child, EditError> {
    match &candidate.contents {
        Payload::Leaf(range) => {
            replay.leaf_page(candidate.number, range.clone(), candidate.source, actual)
        }
        Payload::Branch { children, tail } => replay.branch(
            &Branch {
                number: candidate.number,
                height: candidate.height,
                children,
                tail: *tail,
                source: candidate.source,
            },
            actual,
        ),
    }
}
