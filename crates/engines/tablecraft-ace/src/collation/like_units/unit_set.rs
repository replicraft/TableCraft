//! Immutable complete-key membership and primary-only element pruning.
use crate::{
    InspectError,
    collation::key::{ComparisonKey, Work},
};
use tablecraft_core::limits::Resource;
use tablecraft_expressions::comparison::LikeSpanState;

pub(in crate::collation) fn contains(
    dictionary: &fst::Set<&[u8]>,
    key: &ComparisonKey,
    work: &mut Work<'_>,
) -> Result<bool, InspectError> {
    // The reader traverses at most one edge per complete key byte.
    work.budget
        .charge(Resource::Steps, key.bytes.len() as u64)?;
    work.tick()?;
    let result = dictionary.contains(&key.bytes);
    work.tick()?;
    Ok(result)
}

pub(in crate::collation) fn step(
    dictionary: &fst::Set<&[u8]>,
    key: &ComparisonKey,
    work: &mut Work<'_>,
) -> Result<LikeSpanState, InspectError> {
    work.budget
        .charge(Resource::Steps, key.bytes.len() as u64)?;
    work.tick()?;
    if dictionary.contains(&key.bytes) {
        work.tick()?;
        return Ok(LikeSpanState::Match);
    }
    let fst = dictionary.as_fst();
    let mut node = fst.root();
    for &byte in &key.bytes[..key.primary_length] {
        work.tick()?;
        let Some(index) = node.find_input(byte) else {
            work.tick()?;
            return Ok(LikeSpanState::Impossible);
        };
        node = fst.node(node.transition(index).addr);
    }
    work.tick()?;
    Ok(LikeSpanState::Possible)
}
