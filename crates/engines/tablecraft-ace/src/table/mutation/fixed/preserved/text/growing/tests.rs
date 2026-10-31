//! Independent finish rejection of corrupted placement observations.
use super::super::super::{growth_sources::model, source};
use super::{finish, stage};
use crate::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate,
    table::mutation::variable_update::{
        finish as composition,
        placement::{Page, Plan},
    },
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 30_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn updates(case: &model::Case) -> Vec<SlotScalarUpdate> {
    case.requests
        .iter()
        .map(|request| {
            SlotScalarUpdate::new(
                request.row,
                request
                    .expected
                    .map_or(Value::Null, |value| Value::Text(value.to_owned())),
                request
                    .replacement
                    .map_or(Value::Null, |value| Value::Text(value.to_owned())),
            )
        })
        .collect()
}

fn staged(case: &model::Case) -> (NativeImage, Vec<SlotScalarUpdate>, Plan, usize) {
    let image = image(&case.before);
    let original = source::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    let requests = updates(case);
    let (plan, count) = stage::prepare(
        &image,
        &original,
        "Short_Text",
        &requests,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(image.as_bytes(), case.before);
    assert_eq!(count, case.changed);
    (image, requests, plan, count)
}

fn complete(
    image: NativeImage,
    updates: &[SlotScalarUpdate],
    plan: &Plan,
    count: usize,
) -> Result<Ace16WritePreview, EditError> {
    finish::candidate(
        image,
        "Items",
        "Short_Text",
        updates,
        finish::Observed {
            placement: plan,
            changed_rows: count,
        },
        &mut budget(),
        &|| false,
    )
}

fn rejected(image: NativeImage, updates: &[SlotScalarUpdate], plan: &Plan, count: usize) {
    assert!(matches!(
        complete(image, updates, plan, count),
        Err(EditError::Preservation | EditError::Inspect(InspectError::Malformed))
    ));
}

fn target(plan: &Plan) -> Range<usize> {
    let view = plan.view();
    assert_eq!(view.events.len(), 1);
    view.events[0].target.clone()
}

fn flip(plan: &mut Plan, absolute: usize, bit: u8) {
    let page = plan
        .pages
        .iter_mut()
        .find(|page| page.number as usize == absolute / PAGE_BYTES)
        .expect("Forged byte belongs to an actual staged page");
    page.after[absolute % PAGE_BYTES] ^= bit;
}

fn assert_assembled_source_healthy(original: &NativeImage, plan: &Plan) {
    // This bypasses no source gate: it only demonstrates that a staged forgery
    // is itself readable/admissible, so source validation cannot replace proof.
    let bytes = composition::assemble(original, plan, &[], &mut budget(), &|| false).unwrap();
    let candidate = image(&bytes);
    source::capture(&candidate, "Items", &mut budget(), &|| false).unwrap();
}

#[test]
fn growth_finish_accepts_literal_plans_and_rejects_independent_wrong_counts() {
    for shape in model::SHAPES {
        let case = model::source(shape);
        let (image, requests, plan, count) = staged(&case);
        let result = complete(image, &requests, &plan, count).unwrap();
        assert_eq!(result.candidate_bytes(), case.expected);
        let (image, requests, plan, count) = staged(&case);
        rejected(image, &requests, &plan, count + 1);
    }
}

#[test]
fn growth_finish_rejects_source_valid_cached_value_and_unrelated_page_forgeries() {
    let case = model::source(model::Shape::OriginalPage);
    let (image, requests, mut plan, count) = staged(&case);
    let range = target(&plan);
    let page = plan
        .pages
        .iter()
        .find(|page| page.number as usize == range.start / PAGE_BYTES)
        .unwrap();
    let record =
        &page.after[range.start % PAGE_BYTES..range.end - page.number as usize * PAGE_BYTES];
    let count_at = record.len() - 4;
    assert_eq!(
        u16::from_le_bytes(record[count_at..count_at + 2].try_into().unwrap()),
        5
    );
    let at = count_at - 5 * 2;
    let cached = usize::from(u16::from_le_bytes(record[at..at + 2].try_into().unwrap()));
    // The fifth variable field is a valid stored Int32 calculation capsule.
    // Alter its value, retaining its framing and every native storage role.
    flip(&mut plan, range.start + cached + 20, 1);
    assert_assembled_source_healthy(&image, &plan);
    rejected(image, &requests, &plan, count);

    let (image, requests, mut plan, count) = staged(&case);
    let start = 31 * PAGE_BYTES;
    let before = case.before[start..start + PAGE_BYTES].to_vec();
    let mut after = before.clone();
    after[100] ^= 1;
    // Original sentinel bytes are unrelated to every selected record or map.
    plan.pages.push(Page {
        number: 31,
        before,
        after,
    });
    assert_assembled_source_healthy(&image, &plan);
    rejected(image, &requests, &plan, count);
}

#[test]
fn growth_finish_rejects_root_pointer_and_slot_word_observation_forgeries() {
    let case = model::source(model::Shape::PreForwarded);
    let root_word = usize::from(u16::from_le_bytes(
        case.before[12 * PAGE_BYTES + 14..12 * PAGE_BYTES + 16]
            .try_into()
            .unwrap(),
    ));
    let root = 12 * PAGE_BYTES + (root_word & 0x1fff);
    for (offset, bit) in [(root, 1), (12 * PAGE_BYTES + 15, 0x40)] {
        let (image, requests, mut plan, count) = staged(&case);
        flip(&mut plan, offset, bit);
        rejected(image, &requests, &plan, count);
    }
}

fn map_cell(before: &[u8], page: usize, slot: usize, member: usize) -> usize {
    let word = page * PAGE_BYTES + 14 + slot * 2;
    let start = usize::from(u16::from_le_bytes(
        before[word..word + 2].try_into().unwrap(),
    )) & 0x1fff;
    page * PAGE_BYTES + start + 5 + member / 8
}

#[test]
fn growth_finish_replays_global_owned_candidate_edits_instead_of_trusting_ledger() {
    let case = model::source(model::Shape::EndOfFile);
    for (page, slot) in [(1, 0), (11, 0), (11, 1)] {
        let offset = map_cell(&case.before, page, slot, 32);
        let (image, requests, mut plan, count) = staged(&case);
        let edit = plan
            .append
            .map_edits
            .iter_mut()
            .find(|edit| edit.offset == offset)
            .unwrap();
        edit.after ^= 2;
        rejected(image, &requests, &plan, count);
    }
}

#[test]
fn growth_finish_rejects_correlated_same_width_alternative_request() {
    let case = model::source(model::Shape::OriginalPage);
    let (image, _, plan, count) = staged(&case);
    let alternative = [SlotScalarUpdate::new(
        0,
        Value::Text("alpha".into()),
        Value::Text("雨雨雨雨雨雨".into()),
    )];
    // Both original expectations and the alternate replacement are valid; the
    // independently reconstructed replacement must disagree with staged bytes.
    rejected(image, &alternative, &plan, count);
}
