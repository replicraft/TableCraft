//! Child of the actual normal_fixed_sql_journal integration test; reuse its runner.
use super::{model, run};
use std::fs;
use tablecraft_testkit::fixtures::access::{
    canonical_mixed_before_change as mixed, mutation_fixture,
};

fn refuses_without_effects(before: &[u8], sql: &str) {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("target.accdb");
    let journal = root.path().join("journal");
    fs::write(&database, before).unwrap();
    let output = run(&database, &journal, sql);
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert_eq!(fs::read(&database).unwrap(), before);
    assert!(!journal.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn zero_matches_do_not_skip_ineligible_fixed_target_metadata() {
    let ordinary = model::source(6, model::Recipe::Joint);
    refuses_without_effects(
        &ordinary.before,
        "UPDATE Items SET Flag=True,GuidValue=Null WHERE Int32Value=999",
    );
    // Existing committed ordinary CounterPrimary BEFORE only. Id is genuinely
    // generated/indexed; an empty original row union does not make it writable.
    let counter = mixed::source(6, mixed::Recipe::CounterPrimary);
    refuses_without_effects(
        &counter.before,
        "UPDATE Items SET Score=12,Id=7 WHERE Score=-99",
    );
}

#[test]
fn zero_matches_do_not_grant_jet4_multi_target_capability() {
    let before = mutation_fixture::database();
    refuses_without_effects(&before, "UPDATE Items SET Score=12,Id=4 WHERE Id=999");
}
