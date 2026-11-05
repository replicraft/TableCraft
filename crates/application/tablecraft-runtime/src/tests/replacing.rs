//! Checked Find occurrences feed the ordinary reviewed editing service.
use super::{
    find::{find, search},
    request,
    stored::opened,
};
use crate::{
    Command, EditDraft, Error, FilterComparison, FilterSelectionTarget, FindMatch, FindPosition,
    FindRequest, FindScope, FindSpan, ReplaceError, Response, Worker,
};
use tablecraft_testkit::fixtures::access::{
    canonical_variable_creation as variable, field_presentation, mutation_fixture, wide_properties,
};

fn replace(
    worker: &Worker,
    search: &FindRequest,
    found: &FindPosition,
    text: &str,
) -> Result<EditDraft, Error> {
    let Response::EditDraft(draft) = request(
        worker,
        Command::ReplaceFindMatch {
            request: search.clone(),
            found: found.clone(),
            replacement: text.into(),
            filter: None,
        },
    )?
    else {
        panic!("a checked match yields an ordinary edit draft, not a write");
    };
    Ok(draft)
}

#[test]
fn unicode_occurrence_splices_raw_text_and_literal_wildcards_then_uses_normal_review() {
    let source = variable::source(variable::Case::Unicode);
    let (_directory, path, worker, revision) = opened(&source.bytes);
    let mut search = search(revision, "🙂");
    search.scope = FindScope::Field("TextValue".into());
    search.matching = FindMatch::Any;
    let hit = find(&worker, search.clone()).unwrap().unwrap();
    assert_eq!(
        hit.span,
        Some(FindSpan {
            start_utf16: 2,
            end_utf16: 4
        })
    );
    let mut broken = hit.clone();
    broken.span.as_mut().unwrap().start_utf16 = 3;
    assert!(matches!(
        replace(&worker, &search, &broken, "*?"),
        Err(Error::Replace(ReplaceError::StaleMatch))
    ));
    let draft = replace(&worker, &search, &hit, "*?").unwrap();
    assert_eq!(draft.initial.text, "Ω雪*?a");
    assert!(!draft.initial.is_null);
    assert_eq!(draft.original.text, "Ω雪🙂a");
    let Response::EditReviewed { review, .. } = request(
        &worker,
        Command::ReviewEdit {
            draft_id: draft.id,
            input: draft.initial,
        },
    )
    .unwrap() else {
        panic!("normal planner issues the consuming review");
    };
    assert_eq!(review.changed_rows, 1);
    assert_eq!(review.after.text, "Ω雪*?a");
    assert_eq!(std::fs::read(path).unwrap(), source.bytes);
}

#[test]
fn canonical_integer_partial_replace_and_failed_candidates_preserve_previous_draft() {
    let bytes =
        mutation_fixture::with_rows_and_sort_order(&[(1, 10), (2, 20), (3, 30)], [9, 4, 0, 0]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let mut search = search(revision, "2");
    search.scope = FindScope::Field("Score".into());
    search.matching = FindMatch::Any;
    let hit = find(&worker, search.clone()).unwrap().unwrap();
    assert_eq!(hit.parent_ordinal, 1);
    let draft = replace(&worker, &search, &hit, "7").unwrap();
    assert_eq!(draft.initial.text, "70");
    assert!(replace(&worker, &search, &hit, "not-a-number").is_err());
    let mut changed = search.clone();
    changed.pattern = "9".into();
    assert!(matches!(
        replace(&worker, &changed, &hit, "7"),
        Err(Error::Replace(ReplaceError::StaleMatch))
    ));
    let Response::EditReviewed { review, .. } = request(
        &worker,
        Command::ReviewEdit {
            draft_id: draft.id,
            input: draft.initial,
        },
    )
    .unwrap() else {
        panic!("refused candidates do not replace a prior draft");
    };
    assert_eq!(review.before.text, "20");
    assert_eq!(review.after.text, "70");
    changed = search.clone();
    changed.formatted = true;
    assert!(matches!(
        replace(&worker, &changed, &hit, "7"),
        Err(Error::Replace(ReplaceError::UnsupportedField))
    ));
    let Response::FilteredData(filtered) = request(
        &worker,
        Command::FilterSelection {
            target: FilterSelectionTarget {
                document_revision: revision,
                table: "Items".into(),
                parent_ordinal: 2,
                column: "Score".into(),
                filter_id: None,
                enabled: false,
            },
            comparison: FilterComparison::Equal,
            base: None,
        },
    )
    .unwrap() else {
        panic!("native membership excludes the found record");
    };
    assert!(matches!(
        request(
            &worker,
            Command::ReplaceFindMatch {
                request: search.clone(),
                found: hit.clone(),
                replacement: "42".into(),
                filter: Some((filtered.handle.clone(), true)),
            }
        ),
        Err(Error::Replace(ReplaceError::StaleMatch))
    ));
    let Response::EditDraft(off) = request(
        &worker,
        Command::ReplaceFindMatch {
            request: search.clone(),
            found: hit.clone(),
            replacement: "42".into(),
            filter: Some((filtered.handle, false)),
        },
    )
    .unwrap() else {
        panic!("explicit off mode uses complete source membership");
    };
    assert_eq!(off.initial.text, "420");

    request(&worker, super::open(&path, "")).unwrap();
    assert!(matches!(
        replace(&worker, &search, &hit, "7"),
        Err(Error::StaleDocument)
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn empty_literal_deletes_only_matched_text_and_null_search_is_explicitly_refused() {
    let source = variable::source(variable::Case::Unicode);
    let (_directory, path, worker, revision) = opened(&source.bytes);
    let mut search = search(revision, "bcd");
    search.scope = FindScope::Field("TextValue".into());
    search.matching = FindMatch::Any;
    let hit = find(&worker, search.clone()).unwrap().unwrap();
    let draft = replace(&worker, &search, &hit, "").unwrap();
    assert_eq!(draft.initial.text, "aefgh");
    assert!(!draft.initial.is_null);
    assert_eq!(std::fs::read(path).unwrap(), source.bytes);

    let source = variable::source(variable::Case::NullAndEmpty);
    let (_directory, _path, worker, revision) = opened(&source.bytes);
    search.target.document_revision = revision;
    search.pattern = "Is Null".into();
    search.matching = FindMatch::Whole;
    let hit = find(&worker, search.clone()).unwrap().unwrap();
    assert!(matches!(
        replace(&worker, &search, &hit, "new"),
        Err(Error::Replace(ReplaceError::NullMatch))
    ));
    search.pattern = "\"\"".into();
    let hit = find(&worker, search.clone()).unwrap().unwrap();
    assert_eq!(
        replace(&worker, &search, &hit, "x").unwrap().initial.text,
        "x"
    );
}

#[test]
fn lookup_controls_refuse_even_when_the_raw_numeric_find_was_supported() {
    let property = field_presentation::property(0, 1, 3, &111_i16.to_le_bytes());
    let mut bytes = field_presentation::database(
        &[
            wide_properties::names(&["DisplayControl"]),
            field_presentation::score_group(1, &[property]),
        ],
        false,
    );
    mutation_fixture::set_sort_order(&mut bytes, [9, 4, 0, 0]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    let mut search = search(revision, "20");
    search.scope = FindScope::Field("Score".into());
    let hit = find(&worker, search.clone()).unwrap().unwrap();
    assert!(matches!(
        replace(&worker, &search, &hit, "42"),
        Err(Error::Replace(ReplaceError::UnsupportedControl))
    ));
}
