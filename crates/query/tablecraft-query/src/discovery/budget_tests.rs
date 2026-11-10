use super::budget;
use crate::QueryLimits;
use tablecraft_core::limits::Resource;

#[test]
fn execution_pool_honors_explicit_allowances_without_reintroducing_policy_maxima() {
    let mut work = budget(QueryLimits {
        bytes: 300 * 1024 * 1024,
        rows: 2_000_000,
        steps: 200_000_000,
    });
    work.charge(Resource::Bytes, 256 * 1024 * 1024 + 1).unwrap();
    work.charge(Resource::Rows, 1_000_001).unwrap();
    work.charge(Resource::Steps, 100_000_001).unwrap();
    assert_eq!(work.remaining(Resource::Bytes), 44 * 1024 * 1024 - 1);
    assert_eq!(work.remaining(Resource::Rows), 999_999);
    assert_eq!(work.remaining(Resource::Steps), 99_999_999);

    let mut explicit = budget(QueryLimits {
        bytes: 1,
        rows: 2,
        steps: 3,
    });
    assert!(explicit.charge(Resource::Bytes, 2).is_err());
    assert!(explicit.charge(Resource::Rows, 3).is_err());
    assert!(explicit.charge(Resource::Steps, 4).is_err());
}
