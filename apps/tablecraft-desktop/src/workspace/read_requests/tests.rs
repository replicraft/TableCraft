//! Source-free failure injection at the real desktop issuance-cleanup boundary.
use super::*;
use tablecraft_ui::{Data, Database, EditDraft, EditInput, EditTarget, InputHint};

fn desktop() -> Desktop {
    let mut desktop = Desktop::new(egui::Context::default(), None);
    desktop.state.dismiss_backstage();
    desktop.state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Issuance cleanup".into(),
        format: "ACE16".into(),
        queries: Vec::new(),
        tables: Vec::new(),
    });
    assert!(desktop.state.set_table_data(Data {
        row_ordinals: None,
        source_revision: Some(7),
        table: "Items".into(),
        offset: 0,
        columns: vec!["Amount".into()],
        rows: vec![vec!["12".into()]],
        has_more: false,
    }));
    desktop
}

#[test]
fn rejected_issue_retires_old_cell_review_without_losing_proposed_text_or_stored_data() {
    let mut desktop = desktop();
    let target = EditTarget {
        document_revision: 7,
        table: "Items".into(),
        column: "Amount".into(),
        parent_ordinal: 0,
    };
    desktop.state.begin_edit(target.clone(), 10);
    desktop.state.set_edit_draft(
        10,
        EditDraft {
            id: 20,
            target,
            original: "12".into(),
            initial: EditInput {
                text: "13".into(),
                is_null: false,
            },
            input_hint: InputHint::Int32,
            nullable: true,
        },
    );
    let input = EditInput {
        text: "13".into(),
        is_null: false,
    };
    assert!(desktop.state.can_review_edit(20, &input));
    let issued = desktop
        .worker
        .request(Command::Page {
            table: "inert".into(),
            offset: 0,
        })
        .unwrap();
    desktop.state.begin_edit_review(20, input.clone(), issued);
    desktop.generation = issued;
    desktop.pending = PendingOperation::Edit { request_id: issued };
    desktop.state.busy = true;

    // Inject only the counter-boundary result. The old accepted worker request,
    // its publication revocation and its exact UI terminal router are real.
    exhausted(&desktop.worker, &mut desktop.state);
    desktop.read_request_rejected();

    assert!(!desktop.worker.publication_is_current(issued));
    assert!(matches!(desktop.pending, PendingOperation::Read));
    assert!(!desktop.state.busy);
    assert!(desktop.state.can_review_edit(20, &input));
    assert!(!desktop.state.can_review_edit(
        20,
        &EditInput {
            text: "12".into(),
            is_null: false,
        },
    ));
    assert_eq!(desktop.state.data.as_ref().unwrap().rows[0][0].text, "12");
    assert_eq!(
        desktop.state.error.as_deref(),
        Some(
            desktop
                .state
                .localizer
                .text("objects-read-identities-exhausted")
                .as_str()
        )
    );
}

#[test]
fn repeated_rejection_keeps_terminal_guidance_without_a_phantom_pending_read() {
    let mut desktop = desktop();
    let before = desktop.state.data.as_ref().unwrap().rows[0][0].text.clone();
    for _ in 0..2 {
        exhausted(&desktop.worker, &mut desktop.state);
        desktop.read_request_rejected();
        assert!(matches!(desktop.pending, PendingOperation::Read));
        assert!(!desktop.state.busy);
        assert_eq!(desktop.state.data.as_ref().unwrap().rows[0][0].text, before);
    }
}
