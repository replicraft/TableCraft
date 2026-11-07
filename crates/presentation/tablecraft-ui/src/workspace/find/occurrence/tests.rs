use super::*;
use crate::workspace::find::tests::{page, state};
use crate::workspace::find::{FindMatch, FindPosition, FindSpan};

fn matched() -> State {
    let mut state = state();
    state.find.as_mut().unwrap().matching = FindMatch::Any;
    let request = state.current_find_request().unwrap();
    state.begin_find(1, request.clone());
    assert!(state.set_find_result(
        1,
        request,
        Some(FindPosition {
            parent_ordinal: 200,
            column: "Name".into(),
            span: Some(FindSpan {
                start_utf16: 4,
                end_utf16: 7
            }),
        }),
        Some(page(200)),
    ));
    state
}

#[test]
fn current_receipt_resolves_exact_page_cell_and_options_changes_remove_paint_authority() {
    let state = matched();
    assert_eq!(
        state.current_find_occurrence(),
        Some(Occurrence {
            row: 0,
            column: 1,
            start_utf16: 4,
            end_utf16: 7,
            formatted: false,
        })
    );

    let mut state = matched();
    state.find.as_mut().unwrap().pattern = "other".into();
    assert_eq!(state.current_find_occurrence(), None);

    let mut state = matched();
    state.formatting.date = tablecraft_display::DateFormat::Raw;
    assert_eq!(state.current_find_occurrence(), None);

    let mut state = matched();
    state.find.as_mut().unwrap().formatted = true;
    assert_eq!(state.current_find_occurrence(), None);

    let mut state = matched();
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 0, 0);
    assert_eq!(state.current_find_occurrence(), None);

    let mut state = matched();
    state.data.as_mut().unwrap().source_revision = Some(99);
    assert_eq!(state.current_find_occurrence(), None);

    let mut state = matched();
    state.set_table_data(page(300));
    assert_eq!(state.current_find_occurrence(), None);

    let mut state = matched();
    state.close_find();
    assert_eq!(state.current_find_occurrence(), None);
}
