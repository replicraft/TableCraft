//! Normal history-source widgets drive the same actor and full witness owner.
use super::*;
use crate::workspace::test_support::field_addition::add_field;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    self as off, history,
};
use tablecraft_ui::ObjectTab;

fn fresh_history(
    harness: &mut egui_kittest::Harness<'_, Desktop>,
    previous: &ObjectTab,
    column: &str,
) -> ObjectTab {
    let fresh = harness.state().state.active_object().unwrap();
    assert_eq!(fresh.id, previous.id);
    assert_ne!(
        fresh.target.document_revision,
        previous.target.document_revision
    );
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.field_addition_open());
    assert!(!harness.state().state.field_addition_source_stale());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns.last().unwrap().name, column);
    assert!(data.rows.is_empty());
    assert_eq!(data.source_revision, Some(fresh.target.document_revision));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if harness
            .state()
            .state
            .field_addition_inspection(&fresh)
            .is_some_and(|facts| !facts.0 && facts.1 == Some(true))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "actual history inspection did not complete"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(
        harness
            .state()
            .state
            .can_name_field_addition(&fresh, FieldAdditionKind::Number)
    );
    assert!(
        harness
            .state()
            .state
            .can_name_field_addition(&fresh, FieldAdditionKind::ShortText)
    );
    assert!(harness.state().state.error.is_none());
    fresh
}

#[test]
fn four_sdk_second_afters_use_actual_enabled_commands_and_one_raw_third_continues() {
    for case in [
        history::SourceCase::NumberThenNumber,
        history::SourceCase::MixedThenText,
    ] {
        let before = history::source(case);
        for (column, kind, expected) in [
            (
                "SecondNumber",
                FieldAdditionKind::Number,
                off::append_number(&before, "SecondNumber"),
            ),
            (
                "SecondLabel",
                FieldAdditionKind::ShortText,
                off::append_text(&before, "SecondLabel"),
            ),
        ] {
            // Ordinary Desktop::new performs real source inspection. Existing
            // widget helper refuses to click without its actual per-kind fact.
            let (_directory, path, _, mut harness) = test_support::opened(before.clone());
            let original = harness.state().state.active_object().unwrap();
            add_field(&mut harness, column, kind);
            assert_eq!(std::fs::read(&path).unwrap(), expected);
            assert!(!journal_path(&path).exists());
            let second = fresh_history(&mut harness, &original, column);

            if matches!(case, history::SourceCase::NumberThenNumber)
                && matches!(kind, FieldAdditionKind::ShortText)
            {
                // Third Save is independently modeled raw evidence only; the
                // four complete second AFTERs above are the SDK-backed cases.
                let third = off::append_number(&expected, "ThirdNumber");
                add_field(&mut harness, "ThirdNumber", FieldAdditionKind::Number);
                assert_eq!(std::fs::read(&path).unwrap(), third);
                assert!(!journal_path(&path).exists());
                fresh_history(&mut harness, &second, "ThirdNumber");
            }
        }
    }
}
