//! Real DROP interaction and native actor boundaries, using independent sources.
mod close;
mod failures;
mod ownership;
mod populated;
mod readdition;
mod widgets;

use crate::workspace::{Desktop, test_support::finish_reads};
use egui::{Id, Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_runtime::{FieldDropReview, Response, ReviewedFieldDrop, SaveWorker};
use tablecraft_ui::FluentArgs;

#[derive(Clone, Copy)]
enum Entry {
    Datasheet,
    Design,
}

fn inspect(harness: &mut Harness<'_, Desktop>, column: &str, entry: Entry) {
    let native_number = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .find(|field| field.name == column)
        .unwrap()
        .native_number
        .unwrap();
    assert_eq!(
        harness.state().state.field_drop_available,
        SaveWorker::supports_field_drop()
    );
    assert!(harness.state().state.field_drop_available);
    match entry {
        Entry::Datasheet => {
            harness.get_by_label(column).click();
            harness.run_steps(1);
            harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
            harness.run_steps(2);
            let command = harness.get_by_role_and_label(Role::Button, "Delete Field");
            assert!(!command.accesskit_node().is_disabled());
            command.click_accesskit();
        }
        Entry::Design => {
            harness
                .get_by_role_and_label(Role::Button, "View")
                .click_accesskit();
            harness.run_steps(2);
            harness
                .get_by_role_and_label(Role::Button, "Design view")
                .click_accesskit();
            harness.run_steps(1);
            finish_reads(harness);
            harness.get_by_label(column).click();
            harness.run_steps(2);
            let command = harness.get_by_role_and_label(Role::Button, "Delete Rows");
            assert!(!command.accesskit_node().is_disabled());
            command.click_accesskit();
        }
    }
    harness.run_steps(1);
    finish_reads(harness);
    assert!(harness.state().state.field_drop_confirmation_open());
    let target = harness.state().state.field_drop_target().unwrap();
    assert_eq!(target.column, column);
    assert_eq!(target.native_number, native_number);
    let mut arguments = FluentArgs::new();
    arguments.set("column", column);
    let confirmation = harness
        .state()
        .state
        .localizer
        .format("field-drop-confirm", &arguments);
    harness.get_by_label(&confirmation);
    assert!(!harness.state().state.field_drop_dirty());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "No")
            .is_focused()
    );
}

fn pending(harness: &mut Harness<'_, Desktop>, column: &str) {
    inspect(harness, column, Entry::Datasheet);
    choose(harness, "field_drop_confirmation", "Yes");
    assert!(harness.state().state.field_drop_dirty());
    assert!(harness.state().state.can_review_field_drop());
    assert!(harness.state().editing.field_drop.is_none());
    assert!(harness.state().editing.save.is_none());
}

fn choose(harness: &mut Harness<'_, Desktop>, owner: &str, label: &str) {
    let dialog = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new(owner)))
        .expect("the actual operation-owned dialog is presented");
    let buttons = harness
        .get_all_by_role_and_label(Role::Button, label)
        .filter(|button| dialog.contains_rect(button.rect()))
        .collect::<Vec<_>>();
    assert_eq!(buttons.len(), 1, "one choice belongs to the exact modal");
    assert!(!buttons[0].accesskit_node().is_disabled());
    buttons[0].click_accesskit();
    drop(buttons);
    harness.run_steps(1);
}

fn take_review(harness: &mut Harness<'_, Desktop>) -> (u64, FieldDropReview, ReviewedFieldDrop) {
    let request = harness.state().state.field_drop_review_request().unwrap();
    harness.state_mut().save_field_drop(request);
    assert!(harness.state().state.busy);
    let deadline = Instant::now() + Duration::from_secs(10);
    let (generation, result) = loop {
        if let Some(reply) = harness.state().worker.poll() {
            break reply;
        }
        assert!(
            Instant::now() < deadline,
            "actual DROP review did not finish"
        );
        std::thread::yield_now();
    };
    let Response::FieldDropReviewed { review, capability } = result.unwrap() else {
        panic!("expected the actual consuming native DROP review");
    };
    (generation, review, capability)
}

fn accept_review(harness: &mut Harness<'_, Desktop>) {
    let (generation, review, capability) = take_review(harness);
    harness
        .state_mut()
        .start_reviewed_field_drop(generation, review, capability);
    assert!(harness.state().editing.save.is_some());
}
