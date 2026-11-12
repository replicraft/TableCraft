//! Only explicit application policy resolves absent native wildcard metadata.
use super::{budget, fields, image, raw, syntax};
use crate::editing::sql::{
    NativeMakeTableError, NativeMakeTableOptions,
    prepare_parameterized_native_make_table_with_native_comparison,
};
use std::cell::RefCell;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_expressions::{EvalError, predicates::LikeMode};
use tablecraft_query::{ExecutionOptions, MakeTableError, QueryError};
use tablecraft_testkit::fixtures::access::database_properties;

#[test]
fn native_make_table_keeps_unsupplied_absent_and_unknown_modes_distinct() {
    for mode in [None, Some(0), Some(1), Some(2)] {
        let seed = fields::source(fields::Case::FirstNumber).bytes;
        let original = raw::populated::numbers(&seed, "Items", &[Some(7)]);
        let before = match mode {
            Some(mode) => database_properties::with_native17_properties(
                &original,
                &database_properties::name_auto_correct_disabled_with_query_mode(mode),
            ),
            None => original,
        };
        for fallback in [None, Some(LikeMode::Ansi89)] {
            let result = prepare_parameterized_native_make_table_with_native_comparison(
                image(&before),
                syntax("SELECT CopiedScore INTO Archive FROM Items WHERE 'Apple' LIKE 'a*'"),
                &RefCell::new(budget()),
                &RefCell::new(budget()),
                &mut budget(),
                NativeMakeTableOptions {
                    parameters: &[],
                    execution: ExecutionOptions::default(),
                    absent_like_mode: fallback,
                    snapshot_limits: TableSnapshotLimits::default(),
                },
            );
            if mode == Some(2) || mode.is_none() && fallback.is_none() {
                assert!(
                    matches!(
                        result,
                        Err(NativeMakeTableError::Query(MakeTableError::Query(
                            QueryError::Evaluation(EvalError::LikeModeRequired)
                        )))
                    ),
                    "mode {mode:?}, fallback {fallback:?}"
                );
            } else {
                let staged = result.unwrap();
                assert_eq!(
                    staged.rows().len(),
                    usize::from(mode != Some(1)),
                    "native known mode overrides fallback"
                );
            }
        }
    }
}
