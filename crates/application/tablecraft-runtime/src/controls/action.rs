//! Explicit SQL action phase allowances; no counter is reset during a phase.
use super::{NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS};
use crate::{Error, document::SNAPSHOT_BYTES};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::QueryLimits;

const PROOF_IMAGE_VISITS: u64 = 4;
const PROOF_FRAMING_STEPS: u64 = 4096;
const SOURCE_COMPARE_STREAMS: u64 = 2;

fn image_length(bytes: usize) -> Result<u64, Error> {
    let bytes = u64::try_from(bytes).map_err(|_| Error::SnapshotLimit)?;
    if bytes == 0 || bytes > SNAPSHOT_BYTES {
        return Err(Error::SnapshotLimit);
    }
    Ok(bytes)
}

fn pair_length(before: usize, after: usize) -> Result<u64, Error> {
    image_length(before)?
        .checked_add(image_length(after)?)
        .ok_or(Error::SnapshotLimit)
}

/// Initial intent binding hashes the real immutable BEFORE once. This is a
/// combined native-planning pool, independent from SQL work. Validating the
/// image envelope does not add to the host's representable work counter.
pub(crate) fn action_review_budget(bytes: usize) -> Result<ResourceBudget, Error> {
    image_length(bytes)?;
    Ok(ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS))
}

pub(crate) fn action_query_budget() -> ResourceBudget {
    let limits = action_query_limits();
    ResourceBudget::new(limits.bytes, limits.rows, limits.steps)
}

pub(crate) fn action_query_limits() -> QueryLimits {
    QueryLimits::default()
}

/// Fresh source equality, original binding and rebuilt-image proof. This pool
/// funds no native allocation or row visit, and is used cumulatively once.
pub(crate) fn action_proof_budget(before: usize, after: usize) -> Result<ResourceBudget, Error> {
    let bytes = image_length(before)?
        .checked_mul(SOURCE_COMPARE_STREAMS)
        .ok_or(Error::SnapshotLimit)?;
    let steps = pair_length(before, after)?
        .checked_mul(PROOF_IMAGE_VISITS)
        .and_then(|steps| steps.checked_add(PROOF_FRAMING_STEPS))
        .ok_or(Error::SnapshotLimit)?;
    Ok(ResourceBudget::new(bytes, 0, steps))
}

/// Journal preparation/commit and intent/namespace work, after exact native
/// staging. This is a persistence allowance, not the ordinary planner quota.
/// Image-envelope admission remains separate from cumulative resource accounting.
pub(crate) fn action_persistence_budget(
    before: usize,
    after: usize,
) -> Result<ResourceBudget, Error> {
    pair_length(before, after)?;
    Ok(ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS))
}

/// Recovery starts from retained journal ownership. Untrusted stored lengths do
/// not select its allowance; native framing, ownership and allocation checks
/// remain. Reconstruction and persistence share this recovery pool.
pub(crate) fn action_recovery_budget() -> ResourceBudget {
    ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::limits::Resource;

    #[test]
    fn invalid_lengths_cannot_grant_proof_or_persistence_work() {
        for length in [0, SNAPSHOT_BYTES as usize + 1, usize::MAX] {
            assert!(matches!(
                action_review_budget(length),
                Err(Error::SnapshotLimit)
            ));
            assert!(matches!(
                action_proof_budget(length, 4096),
                Err(Error::SnapshotLimit)
            ));
            assert!(matches!(
                action_proof_budget(4096, length),
                Err(Error::SnapshotLimit)
            ));
            assert!(matches!(
                action_persistence_budget(length, 4096),
                Err(Error::SnapshotLimit)
            ));
            assert!(matches!(
                action_persistence_budget(4096, length),
                Err(Error::SnapshotLimit)
            ));
        }
    }

    #[test]
    fn phase_pools_preserve_exact_proof_boundaries_and_host_allowances() {
        let mut proof = action_proof_budget(8192, 12288).unwrap();
        assert_eq!(proof.remaining(Resource::Rows), 0);
        assert!(proof.charge(Resource::Rows, 1).is_err());
        assert_eq!(proof.remaining(Resource::Bytes), 16384);
        assert!(proof.charge(Resource::Bytes, 16385).is_err());

        let review = action_review_budget(8192).unwrap();
        let query = action_query_budget();
        let persistence = action_persistence_budget(8192, 12288).unwrap();
        let recovery = action_recovery_budget();
        assert_eq!(review.remaining(Resource::Rows), NATIVE_ROWS);
        assert_eq!(review.remaining(Resource::Bytes), NATIVE_BYTES);
        assert_eq!(query.remaining(Resource::Rows), u64::MAX);
        assert_eq!(persistence.remaining(Resource::Steps), u64::MAX);
        assert_eq!(recovery.remaining(Resource::Steps), u64::MAX);
        assert_eq!(persistence.remaining(Resource::Bytes), NATIVE_BYTES);
        assert_eq!(recovery.remaining(Resource::Rows), NATIVE_ROWS);
    }
}
