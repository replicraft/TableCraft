//! A matching public DTO cannot substitute for the opaque captured declaration.
use super::{Desktop, finish_reads, journal_path, raw, test_support};
use crate::workspace::{PendingOperation, editing::field_addition::FieldAdditionHost};
use std::{
    num::NonZeroU8,
    time::{Duration, Instant},
};
use tablecraft_builders::table::{FieldAdditionDeclaration as Declaration, FieldAdditionKind};
use tablecraft_runtime::{Command, Response};

#[test]
fn native_text_width_or_compression_mismatch_refuses_before_worker_start_even_with_matching_dto() {
    for captured in [
        Declaration::ShortText {
            max_utf16_units: NonZeroU8::new(80).unwrap(),
            unicode_compression: true,
        },
        Declaration::ShortText {
            max_utf16_units: NonZeroU8::new(255).unwrap(),
            unicode_compression: false,
        },
    ] {
        let before = raw::mixed::source(raw::mixed::SourceCase::IdOnly);
        let (_directory, path, _, mut harness) = test_support::opened(before.clone());
        finish_reads(&mut harness);
        let tab = harness.state().state.active_object().unwrap();
        let requested = Declaration::ui_default(FieldAdditionKind::ShortText);
        assert!(harness.state_mut().state.begin_field_addition_name(
            &tab,
            "Label".into(),
            requested
        ));
        let request = harness
            .state()
            .state
            .field_addition_review_request()
            .unwrap();
        let generation = harness
            .state()
            .worker
            .request(Command::ReviewAddField {
                document_revision: tab.target.document_revision,
                table: "Table1".into(),
                column: "Label".into(),
                declaration: captured,
            })
            .expect("fresh test worker identity");
        let deadline = Instant::now() + Duration::from_secs(10);
        let (id, result) = loop {
            if let Some(response) = harness.state_mut().worker.poll() {
                break response;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(id, generation);
        let Response::FieldAdditionReviewed {
            mut review,
            capability,
        } = result.unwrap()
        else {
            panic!("actual native unlocked review expected");
        };
        assert_eq!(capability.captured_declaration(), captured);
        // Deliberately make the untrusted data DTO match the UI's request. The
        // genuine capability still carries the other width/compression choice.
        review.declaration = requested;
        let desktop: &mut Desktop = harness.state_mut();
        assert!(
            desktop
                .state
                .begin_field_addition_review(&request, generation)
        );
        desktop.editing.field_addition = Some(FieldAdditionHost {
            request_id: generation,
            request,
            identity: None,
            witness: None,
            terminal: None,
            cleanup: None,
            filters: Vec::new(),
            reopening: false,
            cancelled: false,
        });
        desktop.generation = generation;
        desktop.pending = PendingOperation::FieldAdditionReview {
            request_id: generation,
        };
        desktop.state.busy = true;
        desktop.start_reviewed_field_addition(generation, review, capability);
        assert!(desktop.editing.field_addition.is_none());
        assert!(desktop.editing.save.is_none());
        assert!(!desktop.state.busy);
        assert!(desktop.state.error.is_some());
        assert_eq!(desktop.state.field_addition_name().unwrap().1, "Label");
        assert_eq!(desktop.state.field_addition_declaration(), Some(requested));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());
    }
}
