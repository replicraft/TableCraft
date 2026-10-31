//! Shared service consumers of the one frozen complete-image growth corpus.
use super::{SCOPE, budget, growth_model as model};
use std::io::Cursor;
use tablecraft_access::{
    NativeEditError,
    engineering::{
        AccessEngineeringEditor, EngineeringRequest, ScalarUpdateMode, UpdateMode, preview, sql,
    },
};
use tablecraft_ace::{NativeImage, SlotScalarUpdate};
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, EditableTable};
use tablecraft_sql::{ParseLimits, parse_query};

fn value(text: Option<&str>) -> Value {
    text.map_or(Value::Null, |text| Value::Text(text.into()))
}

fn statement(shape: model::Shape) -> &'static str {
    match shape {
        model::Shape::Mixed | model::Shape::MixedReverse => {
            "UPDATE Items SET [Short_Text]=IIf([ID]=1,'雪雪雪雪雪雪',IIf([ID]=2,NULL,'雪')) WHERE [ID]>=1 AND [ID]<=3"
        }
        model::Shape::Noops => {
            "UPDATE Items SET [Short_Text]=IIf([ID]=1,'alpha',NULL) WHERE [ID]<=2"
        }
        model::Shape::ZeroNull => "UPDATE Items SET [Short_Text]='雪雪雪雪雪雪' WHERE [ID]=2",
        _ => "UPDATE Items SET [Short_Text]='雪雪雪雪雪雪' WHERE [ID]=1",
    }
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .take(8)
        .map(|(at, (left, right))| (at, *left, *right))
        .collect::<Vec<_>>();
    assert!(differences.is_empty(), "first differences {differences:?}");
}

#[test]
fn explicit_projected_text_growth_typed_handles_match_eight_frozen_images() {
    for shape in model::SHAPES {
        let case = model::source(shape);
        let mut allowance = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let editor = AccessEngineeringEditor::new(
            image,
            "Items",
            ScalarUpdateMode::Variable {
                index_growth: false,
            },
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let snapshot = editor.snapshot();
        let column = snapshot
            .columns()
            .iter()
            .position(|field| field.name() == "Short_Text")
            .unwrap();
        let updates = case
            .requests
            .iter()
            .map(|request| {
                assert_eq!(
                    snapshot.rows()[request.row][column],
                    value(request.expected)
                );
                CellUpdate::new(
                    snapshot.row_handle(request.row).unwrap(),
                    value(request.replacement),
                )
            })
            .collect::<Vec<_>>();
        let copy = editor
            .plan_updates("Short_Text", &updates, &mut allowance, &|| false)
            .unwrap();
        assert_image(copy.before_bytes(), &case.before);
        assert_image(copy.candidate_bytes(), &case.expected);
        assert_eq!(copy.affected_rows(), case.changed);
    }
}

#[test]
fn explicit_projected_text_growth_shared_sql_matches_eight_frozen_images() {
    for shape in model::SHAPES {
        let case = model::source(shape);
        let mut allowance = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let query = parse_query(statement(shape), ParseLimits::default()).unwrap();
        let plan = sql::preview_batch_update(
            image,
            query.query(),
            ScalarUpdateMode::Variable {
                index_growth: false,
            },
            &mut allowance,
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), case.requests.len());
        let copy = plan.into_backend();
        assert_image(copy.candidate_bytes(), &case.expected);
        assert_eq!(copy.affected_rows(), case.changed);
    }
}

#[test]
fn projected_growth_dispatch_preserves_stale_expectations_and_other_modes() {
    let case = model::source(model::Shape::OriginalPage);
    let mut allowance = budget();
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        SCOPE,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let stale = [SlotScalarUpdate::new(
        0,
        value(Some("stale")),
        value(Some("雪雪雪雪雪雪")),
    )];
    assert!(
        preview(
            image,
            EngineeringRequest::VariableUpdate {
                table: "Items",
                column: "Short_Text",
                rows: &stale,
                index_growth: false,
            },
            &mut allowance,
            &|| false
        )
        .is_err()
    );

    for mode in [
        ScalarUpdateMode::Default,
        ScalarUpdateMode::Variable { index_growth: true },
        ScalarUpdateMode::LongValue,
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        ScalarUpdateMode::Fixed(UpdateMode::IndexGrowth),
        ScalarUpdateMode::Fixed(UpdateMode::Cascading),
    ] {
        let mut allowance = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let query = parse_query(
            statement(model::Shape::OriginalPage),
            ParseLimits::default(),
        )
        .unwrap();
        assert!(
            sql::preview_batch_update(image, query.query(), mode, &mut allowance, None, &|| false)
                .is_err()
        );
    }
    let create = || {
        let mut allowance = budget();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut allowance,
            &|| false,
        )
        .unwrap();
        AccessEngineeringEditor::new(
            image,
            "Items",
            ScalarUpdateMode::Variable {
                index_growth: false,
            },
            &mut allowance,
            &|| false,
        )
        .unwrap()
    };
    let first = create();
    let second = create();
    let handle = first.snapshot().row_handle(0).unwrap();
    assert!(matches!(
        second.plan_updates(
            "Short_Text",
            &[CellUpdate::new(handle, value(Some("雪雪雪雪雪雪")))],
            &mut budget(),
            &|| false
        ),
        Err(NativeEditError::Snapshot(_))
    ));
}
