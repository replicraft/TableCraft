//! Supported arithmetic failure retains its typed cause in the normal route.
use crate::{
    BeforeEventIntents, EditError, Int32Update, NativeImage, RecoverableEditError,
    inspect_table_schema,
};
use std::{error::Error, io::Cursor};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::EvalError;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    canonical_before_change::{Recipe, source},
    wide_properties,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn supported_division_failure_retains_evaluation_cause_without_a_normal_plan() {
    for header in [5, 6] {
        let case = source(header, Recipe::Sequential);
        let original = wide_properties::text("[Score]+[Old].[Score]");
        let replacement = wide_properties::text("[Score]/([Id]-[Id])  ");
        assert_eq!(original.len(), replacement.len());
        let positions = case
            .before
            .windows(original.len())
            .enumerate()
            .filter_map(|(position, bytes)| (bytes == original.as_slice()).then_some(position))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);
        let start = positions[0];
        let end = start + original.len();
        let mut before = case.before.clone();
        before[start..end].copy_from_slice(&replacement);
        assert_eq!(&before[..start], &case.before[..start]);
        assert_eq!(&before[end..], &case.before[end..]);

        let scope = CatalogScope::new(96);
        let mut work = budget();
        let image =
            NativeImage::capture(&mut Cursor::new(&before), scope, &mut work, &|| false).unwrap();
        let schema =
            inspect_table_schema(&mut image.reader(), scope, "Items", &mut work, &|| false)
                .unwrap();
        let compiled = image
            .prepare_native_before_change_fields(
                schema.object_id(),
                BeforeEventIntents::FixedUpdate {
                    column: "Score",
                    updates: &[],
                },
                &mut work,
                &|| false,
            )
            .unwrap();
        assert!(compiled.final_rows().unwrap().is_empty());
        assert!(compiled.rejection().is_none());
        drop(compiled);

        let error = image
            .plan_recoverable_int32_updates(
                "Items",
                "Score",
                vec![Int32Update::new(0, 10, 17)],
                &mut work,
                &|| false,
            )
            .err()
            .expect("division by zero returns no normal plan");
        let RecoverableEditError::Edit(error) = error else {
            panic!("evaluation is a normal edit error");
        };
        assert_eq!(
            error,
            EditError::BeforeChangeEvaluation(EvalError::DivisionByZero)
        );
        assert_eq!(error.to_string(), "required BeforeChange evaluation failed");
        assert_eq!(error.source().unwrap().to_string(), "division by zero");
        assert_eq!(&before[..start], &case.before[..start]);
        assert_eq!(&before[end..], &case.before[end..]);
    }
}
