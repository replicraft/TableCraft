//! Three ordinary widget saves retain source qualification across fresh revisions.
use super::*;
use crate::workspace::test_support::number_column::add_number;

#[test]
fn ordinary_click_to_add_number_saves_first_second_and_third_without_reopening_manually() {
    let before = raw::source(raw::Source::IdOnly);
    let first = raw::append(&before, raw::Source::IdOnly, "Score");
    let second = raw::repeated::append(&first, "Score2");
    let third = raw::repeated::append(&second, "Third");
    let (_directory, path, _, mut harness) = test_support::opened(before);
    let original_tab = harness.state().state.active_object().unwrap();
    let mut revision = original_tab.target.document_revision;
    for (column, expected) in [("Score", first), ("Score2", second), ("Third", third)] {
        add_number(&mut harness, column);
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.number_column_open());
        assert!(!harness.state().state.number_column_source_stale());
        let fresh = harness.state().state.active_object().unwrap();
        assert_eq!(fresh.id, original_tab.id);
        assert_ne!(fresh.target.document_revision, revision);
        revision = fresh.target.document_revision;
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(data.columns.last().unwrap().name, column);
        assert!(data.rows.is_empty());
        assert_eq!(data.source_revision, Some(revision));
    }
    assert!(harness.state().state.error.is_none());
}
