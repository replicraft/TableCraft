//! Ordinary Click to Add → Command+S → exact witness publication after OFF.
use super::*;
use crate::workspace::test_support::field_addition::add_field;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off as off;

#[test]
fn both_declarations_on_both_off_histories_use_real_widgets_and_full_native_images() {
    for case in [off::SourceCase::Number, off::SourceCase::Mixed] {
        let before = off::source(case);
        for (column, kind, expected) in [
            (
                "AfterOff",
                FieldAdditionKind::Number,
                off::append_number(&before, "AfterOff"),
            ),
            (
                "LabelOff",
                FieldAdditionKind::ShortText,
                off::append_text(&before, "LabelOff"),
            ),
        ] {
            let (_directory, path, _, mut harness) = test_support::opened(before.clone());
            let original = harness.state().state.active_object().unwrap();
            add_field(&mut harness, column, kind);

            assert_eq!(std::fs::read(&path).unwrap(), expected);
            assert!(!journal_path(&path).exists());
            assert!(!harness.state().editing.has_save());
            assert!(!harness.state().state.field_addition_open());
            assert!(!harness.state().state.field_addition_source_stale());
            let fresh = harness.state().state.active_object().unwrap();
            assert_eq!(fresh.id, original.id);
            assert_ne!(
                fresh.target.document_revision,
                original.target.document_revision
            );
            let data = harness.state().state.data.as_ref().unwrap();
            assert_eq!(data.columns.last().unwrap().name, column);
            assert!(data.rows.is_empty());
            assert_eq!(data.source_revision, Some(fresh.target.document_revision));

            // Wait for the real service response on the new revision. There is
            // no synthetic capability, manual availability flag or second Save.
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if harness
                    .state()
                    .state
                    .field_addition_inspection(&fresh)
                    .is_some_and(|inspection| inspection.1 == Some(false))
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "fresh later-event inspection did not settle"
                );
                harness.run_steps(1);
                std::thread::yield_now();
            }
            assert!(
                !harness
                    .state()
                    .state
                    .can_name_field_addition(&fresh, FieldAdditionKind::Number)
            );
            assert!(
                !harness
                    .state()
                    .state
                    .can_name_field_addition(&fresh, FieldAdditionKind::ShortText)
            );
            assert!(harness.state().state.error.is_none());
        }
    }
}
