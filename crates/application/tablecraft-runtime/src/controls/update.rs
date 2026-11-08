//! Explicit UPDATE phase allowances; no counter is reset during a phase.
use super::{NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS};
use crate::{Error, document::SNAPSHOT_BYTES};
use tablecraft_core::limits::ResourceBudget;

const QUERY_BYTES: u64 = 16 * 1024 * 1024;
const QUERY_ROWS: u64 = 10_000;
const QUERY_STEPS: u64 = 1_000_000;
const PROOF_IMAGE_VISITS: u64 = 4;
const PROOF_FRAMING_STEPS: u64 = 4096;
const SOURCE_COMPARE_STREAMS: u64 = 2;
const PERSISTENCE_IMAGE_VISITS: u64 = 8;
const INTENT_AND_NAMESPACE_STEPS: u64 = 16 * 1024 * 1024;

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
/// combined native-planning allowance, independently bounded from SQL work.
/// Native row/byte ceilings remain the existing host limits.
pub(crate) fn update_review_budget(bytes: usize) -> Result<ResourceBudget, Error> {
    let steps = NATIVE_STEPS
        .checked_add(image_length(bytes)?)
        .and_then(|steps| steps.checked_add(PROOF_FRAMING_STEPS))
        .ok_or(Error::SnapshotLimit)?;
    Ok(ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, steps))
}

pub(crate) fn update_query_budget() -> ResourceBudget {
    ResourceBudget::new(QUERY_BYTES, QUERY_ROWS, QUERY_STEPS)
}

/// Fresh source equality, original binding and rebuilt-image proof. This pool
/// funds no native allocation or row visit, and is used cumulatively once.
pub(crate) fn update_proof_budget(before: usize, after: usize) -> Result<ResourceBudget, Error> {
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
/// Byte/row ceilings stay finite; large admitted images can still exhaust them.
pub(crate) fn update_persistence_budget(
    before: usize,
    after: usize,
) -> Result<ResourceBudget, Error> {
    let steps = pair_length(before, after)?
        .checked_mul(PERSISTENCE_IMAGE_VISITS)
        .and_then(|steps| steps.checked_add(INTENT_AND_NAMESPACE_STEPS))
        .and_then(|steps| steps.checked_add(NATIVE_STEPS))
        .ok_or(Error::SnapshotLimit)?;
    Ok(ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, steps))
}

/// Recovery starts from retained journal ownership, before trusted image sizes
/// are decoded. Fund its complete bounded image envelope without trusting an
/// on-disk length to mint allowance. Native framing and allocation limits remain.
/// Reconstruction and persistence share this explicitly named recovery pool.
pub(crate) fn update_recovery_budget() -> ResourceBudget {
    let steps =
        2 * SNAPSHOT_BYTES * PERSISTENCE_IMAGE_VISITS + INTENT_AND_NAMESPACE_STEPS + NATIVE_STEPS;
    ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::limits::Resource;

    #[test]
    fn invalid_lengths_cannot_grant_proof_or_persistence_work() {
        for length in [0, SNAPSHOT_BYTES as usize + 1, usize::MAX] {
            assert!(matches!(
                update_review_budget(length),
                Err(Error::SnapshotLimit)
            ));
            assert!(matches!(
                update_proof_budget(length, 4096),
                Err(Error::SnapshotLimit)
            ));
            assert!(matches!(
                update_proof_budget(4096, length),
                Err(Error::SnapshotLimit)
            ));
            assert!(matches!(
                update_persistence_budget(length, 4096),
                Err(Error::SnapshotLimit)
            ));
            assert!(matches!(
                update_persistence_budget(4096, length),
                Err(Error::SnapshotLimit)
            ));
        }
    }

    #[test]
    fn phase_pools_are_finite_and_preserve_distinct_row_and_byte_boundaries() {
        let mut proof = update_proof_budget(8192, 12288).unwrap();
        assert_eq!(proof.remaining(Resource::Rows), 0);
        assert!(proof.charge(Resource::Rows, 1).is_err());
        assert_eq!(proof.remaining(Resource::Bytes), 16384);
        assert!(proof.charge(Resource::Bytes, 16385).is_err());

        let review = update_review_budget(8192).unwrap();
        let query = update_query_budget();
        let persistence = update_persistence_budget(8192, 12288).unwrap();
        let recovery = update_recovery_budget();
        assert_eq!(review.remaining(Resource::Rows), NATIVE_ROWS);
        assert_eq!(review.remaining(Resource::Bytes), NATIVE_BYTES);
        assert_eq!(query.remaining(Resource::Rows), QUERY_ROWS);
        assert!(persistence.remaining(Resource::Steps) < recovery.remaining(Resource::Steps));
        assert_eq!(persistence.remaining(Resource::Bytes), NATIVE_BYTES);
        assert_eq!(recovery.remaining(Resource::Rows), NATIVE_ROWS);
    }
}
