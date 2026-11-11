//! Cold original reconstruction derives retirement and alias effects from BEFORE.
use super::{Error, Intent, budget, literal};
use crate::{NativeImage, RecoverableNativeIndexDropPlan};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::secondary_index::normal::{self as raw, Population, dropping},
    support::io::journal::MemoryStorage,
};

struct Case {
    source: raw::Source,
    before: Vec<u8>,
    after: Vec<u8>,
    logical: u32,
    physical: u32,
    retired: usize,
}

impl Case {
    fn exclusive(source: raw::Source, logical: u32, physical: u32) -> Self {
        let after = dropping::after(&source);
        let before = source.expected.to_vec();
        let retired = source.index_pages.len() + 1;
        Self {
            source,
            before,
            after,
            logical,
            physical,
            retired,
        }
    }

    fn alias() -> Self {
        let source = raw::protected_companion(false);
        let alias = dropping::surviving_alias(&source, "ScoreAlias");
        Self {
            source,
            before: alias.before,
            after: alias.expected,
            logical: 1,
            physical: 1,
            retired: 0,
        }
    }

    fn compacted() -> Self {
        Self::exclusive(
            dropping::reordered_physical(&raw::protected_companion(true)),
            1,
            0,
        )
    }

    fn wire(&self) -> Vec<u8> {
        literal(
            &self.before,
            [self.source.table, self.source.index_name],
            self.source.target_definition,
            self.logical,
            self.physical,
        )
    }

    fn original(&self) -> Intent {
        Intent::decode(&self.wire(), &mut budget(), &|| false).unwrap()
    }

    fn plan(&self) -> RecoverableNativeIndexDropPlan {
        image(&self.before, 30910)
            .plan_recoverable_retained_index_drop(
                self.source.table,
                self.source.index_name,
                &mut budget(),
                &|| false,
            )
            .unwrap()
    }
}

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete retained length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn original_drop_reconstructs_root_retirement_alias_preservation_and_physical_compaction() {
    let cases = [
        Case::exclusive(raw::source(Population::Empty, false), 0, 0),
        Case::exclusive(raw::source(Population::NullableDuplicate, true), 0, 0),
        Case::exclusive(raw::source(Population::MultiLevel, false), 0, 0),
        Case::exclusive(raw::protected_companion(true), 1, 1),
        Case::alias(),
        Case::compacted(),
    ];
    for case in cases {
        let plan = case.plan();
        same(plan.before_bytes(), &case.before);
        same(plan.after_bytes(), &case.after);
        assert_eq!(plan.retired_pages(), case.retired);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            case.wire()
        );
        assert_eq!(plan.intent().logical_index_number(), case.logical);
        assert_eq!(plan.intent().physical_index_number(), case.physical);
        let shared = plan.after_shared_bytes();
        assert_eq!(shared.as_ptr(), plan.after_bytes().as_ptr());
        let rebuilt = plan
            .requalify_with_budgets(
                image(&case.before, 30911),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(rebuilt.after_bytes(), &case.after);
        drop(rebuilt);
        same(&shared, &case.after);
        for page in case
            .source
            .index_pages
            .iter()
            .copied()
            .chain([case.source.map_host])
        {
            let start = page as usize * 4096;
            if case.retired == 0 {
                assert_eq!(
                    &case.after[start..start + 4096],
                    &case.before[start..start + 4096]
                );
            } else {
                assert!(
                    case.after[start..start + 4096]
                        .iter()
                        .all(|byte| *byte == 0)
                );
            }
        }
    }
}

#[test]
fn pending_mixed_committed_and_settled_drop_keep_the_same_checked_v3_owner() {
    for case in [
        Case::exclusive(raw::source(Population::ThreeLeaves, true), 0, 0),
        Case::alias(),
        Case::compacted(),
    ] {
        for state in 0..3 {
            let mut target = MemoryStorage::new(case.before.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = case
                .plan()
                .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if state == 2 {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                if state == 1 {
                    *target.bytes.get_mut() = case.after.to_vec();
                    for (page, old) in case.before.as_chunks::<4096>().0.iter().enumerate() {
                        if page % 2 == 0 {
                            target.bytes.get_mut()[page * 4096..(page + 1) * 4096]
                                .copy_from_slice(old);
                        }
                    }
                }
            }
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let qualified = case
                .original()
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30920),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                qualified.state(),
                if state == 2 {
                    ResizableState::Committed
                } else {
                    ResizableState::Pending
                }
            );
            assert_eq!(
                qualified
                    .recover_retained(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if state == 2 {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            same(
                target.bytes.get_ref(),
                if state == 2 {
                    &case.after
                } else {
                    &case.before
                },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            case.original()
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30921),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                target.writes, writes,
                "settled confirmation must not replay retirement"
            );
        }
    }
}

#[test]
fn stale_identity_and_forged_retirement_or_protected_after_grant_no_effects() {
    let case = Case::compacted();
    let mut stale = case.before.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        case.plan()
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, 0));
    same(target.bytes.get_ref(), &stale);
    assert!(matches!(
        case.original()
            .verify_before(&stale, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));

    for offset in [24, 56, 60, 64, 68, 80 + case.source.table.len()] {
        let mut wrong = case.wire();
        if offset == 60 {
            let other = if case.source.target_definition == 1 {
                2_u32
            } else {
                1_u32
            };
            wrong[60..64].copy_from_slice(&other.to_le_bytes());
        } else if offset >= 80 {
            // A valid but different requested name cannot borrow this DROP effect.
            wrong[offset] = b'X';
        } else {
            wrong[offset] ^= 1;
        }
        let intent = Intent::decode(&wrong, &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("canonical identity tamper offset {offset}: {error}"));
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &case.before,
                &case.after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            intent
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30930),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "identity offset {offset}"
        );
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &case.before);
    }

    for case in [case, Case::alias()] {
        for offset in [
            3 * 4096 + 3000,
            case.source.target_definition as usize * 4096 + 3000,
            case.source.root as usize * 4096 + 4000,
            case.source.map_host as usize * 4096 + 4000,
        ] {
            let mut forged = case.after.to_vec();
            forged[offset] ^= 1;
            let mut target = MemoryStorage::new(case.before.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            drop(
                journal::prepare_resizable_v3_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &forged,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            assert!(
                case.original()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(30931),
                        &mut budget(),
                        &|| false
                    )
                    .is_err(),
                "forged AFTER byte {offset}"
            );
            assert_eq!(target.writes, 0);
        }
    }
}

#[test]
fn equal_length_drop_still_refuses_v1_v2_unpaid_or_cancelled_qualification() {
    let case = Case::exclusive(raw::source(Population::NullableDuplicate, false), 0, 0);
    assert_eq!(case.before.len(), case.after.len());
    assert!(
        case.plan()
            .requalify_with_budgets(
                image(&case.before, 30940),
                &mut budget(),
                &mut tablecraft_core::limits::ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    for version in [1, 2, 3] {
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        if version == 1 {
            drop(
                journal::prepare_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &case.after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        } else if version == 2 {
            drop(
                journal::prepare_resizable_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &case.after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        } else {
            drop(
                journal::prepare_resizable_v3_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &case.after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        }
        let writes = journal_file.writes;
        if version < 3 {
            assert!(journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).is_err());
        } else {
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                case.original().qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30941),
                    &mut budget(),
                    &|| true
                ),
                Err(Error::Inspect(crate::InspectError::Cancelled))
            ));
        }
        assert_eq!(journal_file.writes, writes);
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &case.before);
    }
}
