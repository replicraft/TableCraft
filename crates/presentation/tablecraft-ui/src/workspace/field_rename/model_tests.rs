use super::model::{Model, Observation, Phase};
use super::{FieldRenameRequest, FieldRenameTarget};
use crate::workspace::{ObjectKind, ObjectTab, ObjectTarget};

fn target() -> FieldRenameTarget {
    FieldRenameTarget {
        tab: ObjectTab {
            id: 17,
            target: ObjectTarget {
                document_revision: 29,
                kind: ObjectKind::Table {
                    name: "Table1".into(),
                },
            },
        },
        column: "Score".into(),
        native_number: 7,
    }
}

fn ready() -> (Model, FieldRenameTarget) {
    let target = target();
    let mut model = Model::default();
    assert!(model.begin(target.copy().unwrap(), "Score".into()));
    assert!(model.bind_inspection(&target, 80));
    assert!(model.observed(&target, 80, true, Some(5)));
    (model, target)
}

fn request(model: &Model) -> FieldRenameRequest {
    let draft = model.draft.as_ref().unwrap();
    FieldRenameRequest {
        target: draft.target.copy().unwrap(),
        draft_revision: draft.revision,
        new_name: super::copy_text(&draft.proposed).unwrap(),
    }
}

#[test]
fn inspection_binds_true_target_and_actual_id_while_preserving_newer_text() {
    let target = target();
    let mut model = Model::default();
    assert!(model.begin(target.copy().unwrap(), "Score".into()));
    assert!(model.bind_inspection(&target, 80));
    assert!(model.replace(&target, "Total".into()));
    let before = request(&model);
    assert!(!model.observed(&target, 79, true, Some(5)));
    let mut other = target.copy().unwrap();
    other.native_number = 0;
    assert!(!model.observed(&other, 80, true, Some(5)));
    assert!(!model.observed(&target, 80, true, Some(4)));
    assert_eq!(request(&model), before);
    assert!(model.observed(&target, 80, true, Some(5)));
    assert_eq!(request(&model), before);
    assert!(model.profile_fits());
}

#[test]
fn unsupported_shape_and_source_refusal_keep_proposed_text_and_revision() {
    let (mut model, target) = ready();
    for name in ["Longer", "Scoré", ""] {
        assert!(model.replace(&target, name.into()));
        assert!(!model.profile_fits());
        let proposed = request(&model);
        assert!(!model.begin_review(&proposed, 90));
        assert_eq!(request(&model), proposed);
    }
    assert!(model.replace(&target, "Total".into()));
    let proposed = request(&model);
    assert!(model.retry_inspection(&target));
    assert!(model.bind_inspection(&target, 81));
    assert!(model.observed(&target, 81, false, None));
    assert!(!model.profile_fits());
    assert_eq!(request(&model), proposed);
}

#[test]
fn stale_and_cancelled_review_callbacks_never_replace_or_discard_the_name() {
    let (mut model, target) = ready();
    let old = request(&model);
    assert!(model.replace(&target, "Total".into()));
    assert!(!model.begin_review(&old, 90));
    let current = request(&model);
    assert!(model.begin_review(&current, 91));
    assert!(!model.replace(&target, "Other".into()));
    assert!(!model.release_review(&current, 90));
    assert!(!model.discard(&target, current.draft_revision));
    assert!(model.release_review(&current, 91));
    assert_eq!(request(&model), current);
    assert!(model.begin_review(&current, 92));
    assert!(model.reviewed(&current, 92, 900));
    assert!(!model.release_accepted(&current, 92, 901));
    assert!(model.release_accepted(&current, 92, 900));
    assert_eq!(model.draft.as_ref().unwrap().phase, Phase::Naming);
    assert_eq!(request(&model), current);
}

#[test]
fn exact_noop_and_case_only_still_need_positive_source_and_real_review() {
    let target = target();
    let mut model = Model::default();
    assert!(model.begin(target.copy().unwrap(), "Score".into()));
    assert!(!model.dirty());
    let original = request(&model);
    assert!(!model.begin_review(&original, 90));
    assert!(model.bind_inspection(&target, 80));
    assert!(model.observed(&target, 80, true, Some(5)));
    assert!(model.begin_review(&original, 90));
    assert!(model.release_review(&original, 90));
    assert!(model.replace(&target, "score".into()));
    assert!(model.dirty());
    assert!(model.begin_review(&request(&model), 91));
}

#[test]
fn pending_error_interruption_retry_and_discard_are_exact_and_memory_accounted() {
    let target = target();
    let mut model = Model::default();
    let mut proposed = String::with_capacity(512);
    proposed.push_str("Score");
    assert!(model.begin(target.copy().unwrap(), proposed));
    let initial = model.heap_bytes().unwrap();
    assert!(initial >= 512 + "Table1".len() + "Score".len());
    assert!(model.bind_inspection(&target, 80));
    assert!(!model.retry_inspection(&target));
    let mut message = String::with_capacity(1024);
    message.push_str("read failed");
    let capacity = message.capacity();
    assert!(model.inspection_failed(&target, 80, message));
    assert_eq!(model.heap_bytes().unwrap(), initial + capacity);
    assert!(model.retry_inspection(&target));
    assert!(model.bind_inspection(&target, 81));
    assert!(model.inspection_interrupted(&target, 81));
    assert!(matches!(
        model.draft.as_ref().unwrap().observation,
        Observation::Interrupted
    ));
    assert_eq!(model.heap_bytes().unwrap(), initial);
    assert!(model.replace(&target, "Total".into()));
    assert!(!model.discard(&target, 0));
    assert!(model.discard(&target, 1));
    assert_eq!(model.heap_bytes(), Some(0));
}
