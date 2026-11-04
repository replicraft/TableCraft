mod editing;
mod navigation;
mod saved;

use crate::file_dialog::{FileDialog, SelectedFile};
use editing::Editing;
use std::path::PathBuf;
use tablecraft_runtime::{Command, ContentsTarget as RuntimeContentsTarget, Response, Worker};
use tablecraft_ui::{
    Action, ContentsTarget, Data, Database, Field, RecordTarget, SavedQuery, SavedQueryTarget,
    Schema, State, Table,
};
use zeroize::Zeroize;

enum PendingOperation {
    Read,
    Query,
    SavedInspection {
        target: SavedQueryTarget,
        request_id: u64,
    },
    SavedRun {
        target: SavedQueryTarget,
        request_id: u64,
    },
    Record {
        target: RecordTarget,
        request_id: u64,
    },
    Contents {
        target: ContentsTarget,
        request_id: u64,
    },
    Edit {
        request_id: u64,
    },
}

pub(super) struct Desktop {
    state: State,
    worker: Worker,
    generation: u64,
    pending: PendingOperation,
    file_dialog: FileDialog,
    selected_file: Option<SelectedFile>,
    editing: Editing,
}

impl Desktop {
    pub(super) fn new(ctx: egui::Context, path: Option<PathBuf>) -> Self {
        let editing = Editing::new(ctx.clone());
        let worker = Worker::new(move || ctx.request_repaint());
        let mut app = Self {
            state: State::default(),
            worker,
            generation: 0,
            pending: PendingOperation::Read,
            file_dialog: FileDialog::default(),
            selected_file: None,
            editing,
        };
        app.state.file_picker_available = true;
        if let Some(path) = path {
            app.select_file(path);
            app.open();
        }
        app
    }

    fn submit(&mut self, command: Command) {
        if self.editing.has_save() {
            return;
        }
        let contents = match &command {
            Command::InspectCollection { target } => {
                Some(crate::view_models::contents_target(target))
            }
            _ => None,
        };
        if let Some(target) = &contents {
            if !self.state.can_view_contents(target) {
                self.state.error = Some("The collection no longer belongs to this view.".into());
                return;
            }
        } else {
            self.state.close_contents();
        }
        if matches!(&command, Command::Describe { .. }) {
            self.state.prepare_schema_view();
        }
        let query = matches!(&command, Command::Query { .. });
        self.generation = self.worker.request(command);
        self.pending = if let Some(target) = contents {
            self.state.begin_contents(target.clone(), self.generation);
            PendingOperation::Contents {
                target,
                request_id: self.generation,
            }
        } else if query {
            PendingOperation::Query
        } else {
            PendingOperation::Read
        };
        self.state.busy = true;
        self.state.error = None;
    }

    fn select_file(&mut self, path: PathBuf) {
        let selected = SelectedFile::new(path);
        self.state.path = selected.display().to_owned();
        self.selected_file = Some(selected);
    }

    fn poll_file_dialog(&mut self) {
        if let Some(selection) = self.file_dialog.poll() {
            self.file_selected(selection);
        }
    }

    fn file_selected(&mut self, selection: Option<PathBuf>) {
        if self.editing.has_save() {
            return;
        }
        if let Some(path) = selection {
            self.select_file(path);
            self.open();
        }
    }

    fn open(&mut self) {
        if self.editing.has_save() {
            return;
        }
        self.editing.clear_review();
        self.editing.clear_refresh();
        self.state.show_open();
        let path = match self
            .selected_file
            .as_ref()
            .and_then(|selected| selected.path_for(&self.state.path))
        {
            Some(path) => path.to_path_buf(),
            None => {
                self.selected_file = None;
                PathBuf::from(&self.state.path)
            }
        };
        let password = std::mem::take(&mut self.state.password);
        self.state.database = None;
        self.state.data = None;
        self.state.query_data = None;
        self.state.clear_schema();
        self.state.close_contents();
        self.submit(Command::Open { path, password });
    }

    fn poll(&mut self) {
        while let Some((generation, result)) = self.worker.poll() {
            if generation != self.generation {
                continue;
            }
            self.state.busy = false;
            match result {
                Ok(Response::SavedQuery { target, parameters }) => {
                    self.publish_saved_query(generation, target, parameters);
                }
                Ok(Response::EditDraft(draft)) => self.publish_edit_draft(generation, draft),
                Ok(Response::EditReviewed { review, capability }) => {
                    self.publish_edit_review(generation, review, capability);
                }
                Ok(Response::Details {
                    table,
                    declared_rows,
                    fields,
                }) => self.state.set_schema(Schema {
                    table,
                    declared_rows,
                    fields: fields
                        .into_iter()
                        .map(|field| Field {
                            name: field.name,
                            number: field.number,
                            native_type: field.native_type,
                            storage_bytes: field.storage_bytes,
                            fixed: field.fixed,
                            flags: field.flags,
                            extended_flags: field.extended_flags,
                            scalar_type: field.scalar_type,
                            reader_nullable: field.reader_nullable,
                            decimal_parameters: field.decimal_parameters,
                            complex_type_id: field.complex_type_id,
                        })
                        .collect(),
                }),
                Ok(Response::Opened {
                    document_revision,
                    title,
                    format,
                    tables,
                    queries,
                }) => {
                    self.state.database = Some(Database {
                        document_revision,
                        title,
                        format,
                        queries: queries
                            .into_iter()
                            .map(|query| SavedQuery {
                                object_id: query.object_id,
                                name: query.name,
                                system: query.system,
                            })
                            .collect(),
                        tables: tables
                            .into_iter()
                            .map(|table| Table {
                                name: table.name,
                                system: table.system,
                                storage: table.storage,
                            })
                            .collect(),
                    });
                    self.state.dismiss_backstage();
                    self.refresh_after_save(generation);
                }
                Ok(Response::Data {
                    source_revision,
                    table,
                    offset,
                    columns,
                    rows,
                    has_more,
                }) => {
                    let data = Data {
                        source_revision,
                        table,
                        offset,
                        columns,
                        rows: crate::view_models::rows(rows),
                        has_more,
                    };
                    match &self.pending {
                        PendingOperation::Query => self.state.set_query_data(data),
                        PendingOperation::SavedRun { target, request_id } => {
                            self.state
                                .set_saved_query_data(*request_id, target.clone(), data);
                        }
                        PendingOperation::Record { target, request_id } => {
                            self.state
                                .set_record_data(*request_id, target.clone(), data);
                        }
                        _ => self.state.set_table_data(data),
                    }
                }
                Ok(Response::Contents {
                    target,
                    kind,
                    assigned,
                    columns,
                    rows,
                }) => {
                    if let PendingOperation::Contents { request_id, .. } = &self.pending {
                        let target = crate::view_models::contents_target(&target);
                        let data = Data {
                            table: target.column.clone(),
                            source_revision: None,
                            offset: 0,
                            columns,
                            rows: crate::view_models::rows(rows),
                            has_more: false,
                        };
                        self.state.set_contents(
                            *request_id,
                            target,
                            crate::view_models::collection_kind(kind),
                            assigned,
                            data,
                        );
                    }
                }
                Err(error) => match &self.pending {
                    PendingOperation::SavedInspection { target, request_id }
                    | PendingOperation::SavedRun { target, request_id } => {
                        self.state
                            .fail_saved_query(*request_id, target.clone(), error.to_string());
                    }
                    PendingOperation::Record { request_id, .. } => {
                        self.state
                            .fail_record_navigation(*request_id, error.to_string());
                    }
                    PendingOperation::Edit { request_id } => {
                        self.state.fail_edit(*request_id, error.to_string());
                    }
                    PendingOperation::Contents { target, request_id } => {
                        self.state
                            .fail_contents(*request_id, target.clone(), error.to_string());
                    }
                    _ => self.state.error = Some(error.to_string()),
                },
            }
        }
    }

    fn close_contents(&mut self, request_id: u64) {
        if matches!(&self.pending, PendingOperation::Contents { request_id: current, .. } if *current == request_id)
            && self.generation == request_id
        {
            self.state.close_contents();
            if self.state.busy {
                self.worker.cancel();
                self.state.busy = false;
            }
            self.pending = PendingOperation::Read;
        }
    }
}

impl eframe::App for Desktop {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // eframe also calls logic for repaint requests while the viewport is
        // hidden. Task completion must not depend on drawing a visible frame.
        self.poll_file_dialog();
        self.poll();
        self.poll_saves();
        self.guard_save_close(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let actions = self.state.show(ui);
        // A manual edit ends the native selection binding, even if the user
        // later types the same lossy display string again.
        if self
            .selected_file
            .as_ref()
            .is_some_and(|selected| selected.path_for(&self.state.path).is_none())
        {
            self.selected_file = None;
        }
        for action in actions {
            match action {
                Action::ChooseFile => self.file_dialog.open(frame, ui.ctx()),
                Action::Open => self.open(),
                Action::Page { table, offset } => self.submit(Command::Page { table, offset }),
                Action::NavigateRecord { target } => self.navigate_record(target),
                Action::ViewSchema { table } => self.submit(Command::Describe { table }),
                Action::ViewContents { target } => self.submit(Command::InspectCollection {
                    target: RuntimeContentsTarget {
                        document_revision: target.document_revision,
                        table: target.table,
                        column: target.column,
                        parent_ordinal: target.parent_ordinal,
                    },
                }),
                Action::CloseContents { request_id } => self.close_contents(request_id),
                Action::InspectSavedQuery { target } => self.inspect_saved_query(target),
                Action::RunSavedQuery { target, parameters } => {
                    self.run_saved_query(target, parameters);
                }
                Action::Query => self.submit(Command::Query {
                    sql: self.state.sql.clone(),
                }),
                Action::BeginEdit { target } => self.begin_edit(target),
                Action::ReviewEdit { draft_id, input } => self.review_edit(draft_id, input),
                Action::ApplyEdit { review_id } => self.apply_edit(review_id),
                Action::CloseEdit { request_id, .. } => self.close_edit(request_id),
                Action::CancelEditSave { save_id } => self.cancel_save(save_id),
                Action::RecoverEditSave { save_id } => self.resolve_save(save_id, true),
                Action::ReleaseEditSave { save_id } => self.resolve_save(save_id, false),
                Action::AcknowledgeEditSave { save_id } => self.acknowledge_save(save_id),
                Action::Cancel => {
                    if self.editing.has_save() {
                        self.cancel_current_save();
                        continue;
                    }
                    if let PendingOperation::Contents { request_id, .. } = &self.pending {
                        self.close_contents(*request_id);
                    } else {
                        self.worker.cancel();
                        self.state.busy = false;
                        match &self.pending {
                            PendingOperation::Record { request_id, .. } => {
                                self.state.fail_record_navigation(
                                    *request_id,
                                    "Operation cancelled.".into(),
                                );
                            }
                            PendingOperation::SavedInspection { target, request_id }
                            | PendingOperation::SavedRun { target, request_id } => {
                                self.state.fail_saved_query(
                                    *request_id,
                                    target.clone(),
                                    "Operation cancelled.".into(),
                                );
                            }
                            _ => self.state.error = Some("Operation cancelled.".into()),
                        }
                    }
                }
            }
        }
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        self.state.password.zeroize();
        self.worker.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::App;
    use std::path::Path;
    use std::time::{Duration, Instant};
    use tablecraft_testkit::fixtures::access::attachment_modern;

    fn finish(desktop: &mut Desktop, context: &egui::Context, frame: &mut eframe::Frame) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while desktop.state.busy && Instant::now() < deadline {
            desktop.logic(context, frame);
            std::thread::yield_now();
        }
        assert!(!desktop.state.busy, "desktop task did not finish");
    }

    #[test]
    fn task_completion_is_consumed_without_drawing_a_visible_frame() {
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), None);
        let mut frame = eframe::Frame::_new_kittest();
        desktop.submit(Command::Query {
            sql: "SELECT 1".into(),
        });
        finish(&mut desktop, &context, &mut frame);
        assert_eq!(desktop.state.error.as_deref(), Some("no database is open"));
    }

    #[test]
    fn backstage_stays_open_on_failure_and_closes_only_after_open_succeeds() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), None);
        let mut frame = eframe::Frame::_new_kittest();
        assert!(desktop.state.backstage_visible());

        // A workspace picker selection must expose the Open surface immediately,
        // including when the asynchronous open subsequently fails.
        desktop.state.dismiss_backstage();
        desktop.state.password = "pending credential".into();
        desktop.file_selected(Some(directory.path().join("missing.accdb")));
        assert!(desktop.state.backstage_visible());
        assert!(desktop.state.busy);
        assert!(desktop.state.password.is_empty());
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.backstage_visible());
        assert!(desktop.state.database.is_none());
        assert!(desktop.state.error.is_some());

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb");
        desktop.file_selected(Some(path));
        assert!(desktop.state.backstage_visible());
        assert!(desktop.state.busy);
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.database.is_some());
        assert!(desktop.state.error.is_none());
        assert!(!desktop.state.backstage_visible());

        // Entering Backstage does not cancel a task, and a query response does
        // not dismiss it as though another document had just opened.
        desktop.submit(Command::Query {
            sql: "SELECT 17 AS [Kept]".into(),
        });
        let generation = desktop.generation;
        desktop.state.show_open();
        assert_eq!(desktop.generation, generation);
        assert!(desktop.state.busy);
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.backstage_visible());
        assert_eq!(
            desktop.state.query_data.as_ref().unwrap().rows[0][0].text,
            "17"
        );
    }

    #[test]
    fn cancelling_the_picker_preserves_the_document_views_and_open_inputs() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb");
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
        let mut frame = eframe::Frame::_new_kittest();
        finish(&mut desktop, &context, &mut frame);
        desktop.submit(Command::Page {
            table: "tblContacts".into(),
            offset: 0,
        });
        finish(&mut desktop, &context, &mut frame);
        desktop.submit(Command::Query {
            sql: "SELECT 17 AS [Kept]".into(),
        });
        finish(&mut desktop, &context, &mut frame);
        desktop.submit(Command::Describe {
            table: "tblContacts".into(),
        });
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.error.is_none());

        desktop.state.show_open();
        desktop.state.password = "unsubmitted credential".into();
        desktop.state.error = Some("retained diagnostic".into());
        let generation = desktop.generation;
        let revision = desktop.state.database.as_ref().unwrap().document_revision;
        let table_rows = desktop.state.data.as_ref().unwrap().rows.as_ptr();
        let query_rows = desktop.state.query_data.as_ref().unwrap().rows.as_ptr();
        let fields = desktop.state.schema.as_ref().unwrap().fields.as_ptr();

        desktop.file_selected(None);
        assert_eq!(desktop.generation, generation);
        assert!(!desktop.state.busy);
        assert!(desktop.state.backstage_visible());
        assert_eq!(desktop.state.password, "unsubmitted credential");
        assert_eq!(desktop.state.error.as_deref(), Some("retained diagnostic"));
        assert_eq!(
            desktop.state.database.as_ref().unwrap().document_revision,
            revision
        );
        assert_eq!(
            desktop.state.data.as_ref().unwrap().rows.as_ptr(),
            table_rows
        );
        assert_eq!(
            desktop.state.query_data.as_ref().unwrap().rows.as_ptr(),
            query_rows
        );
        assert_eq!(
            desktop.state.schema.as_ref().unwrap().fields.as_ptr(),
            fields
        );
        assert_eq!(
            desktop
                .selected_file
                .as_ref()
                .unwrap()
                .path_for(&desktop.state.path),
            Some(path.as_path())
        );

        desktop.state.dismiss_backstage();
        desktop.file_selected(None);
        assert!(!desktop.state.backstage_visible());
        assert_eq!(desktop.generation, generation);
    }

    #[test]
    fn schema_response_is_adapted_and_cleared_when_the_document_changes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb");
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), Some(path));
        let mut frame = eframe::Frame::_new_kittest();
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.database.is_some());

        desktop.submit(Command::Describe {
            table: "tblContacts".to_owned(),
        });
        finish(&mut desktop, &context, &mut frame);
        let schema = desktop.state.schema.as_ref().expect("schema response");
        assert_eq!(schema.table, "tblContacts");
        assert_eq!(schema.fields[0].name, "ID");
        assert_eq!(schema.fields[0].scalar_type.as_deref(), Some("Int32"));
        assert!(desktop.state.data.is_none());

        // Reopening clears the old schema before the next request completes.
        desktop.submit(Command::Describe {
            table: "tblContacts".to_owned(),
        });
        desktop.open();
        assert!(desktop.state.schema.is_none());
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.database.is_some());
        assert!(desktop.state.schema.is_none());
    }

    #[test]
    fn query_null_and_literal_null_keep_distinct_cell_facts_through_the_host() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb");
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), Some(path));
        let mut frame = eframe::Frame::_new_kittest();
        finish(&mut desktop, &context, &mut frame);
        desktop.submit(Command::Query {
            sql: "SELECT NULL AS [Absent], 'NULL' AS [Label], True AS [Yes], False AS [No], 'false' AS [Text]".into(),
        });
        finish(&mut desktop, &context, &mut frame);

        let data = desktop.state.query_data.as_ref().expect("query result");
        assert_eq!(data.rows[0][0].text, "NULL");
        assert_eq!(data.rows[0][1].text, "NULL");
        assert!(matches!(
            data.rows[0][0].kind,
            tablecraft_ui::CellKind::Scalar {
                is_null: true,
                stored_calculation: false,
                boolean: None,
                display_control: tablecraft_ui::DisplayControl::Uninspected
            }
        ));
        assert!(matches!(
            data.rows[0][1].kind,
            tablecraft_ui::CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
                boolean: None,
                display_control: tablecraft_ui::DisplayControl::Uninspected
            }
        ));
        for (index, boolean) in [(2, Some(true)), (3, Some(false)), (4, None)] {
            assert!(matches!(
                data.rows[0][index].kind,
                tablecraft_ui::CellKind::Scalar {
                    is_null: false,
                    stored_calculation: false,
                    boolean: actual,
                display_control: tablecraft_ui::DisplayControl::Uninspected
                } if actual == boolean
            ));
        }
        assert_eq!(data.rows[0][3].text, data.rows[0][4].text);
        assert!(desktop.state.data.is_none());
    }

    #[test]
    fn collection_metadata_roundtrip_keeps_source_and_dialog_requests_separate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("attachments.accdb");
        let original = attachment_modern::database(6);
        std::fs::write(&path, &original).unwrap();
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
        let mut frame = eframe::Frame::_new_kittest();
        finish(&mut desktop, &context, &mut frame);
        let revision = desktop.state.database.as_ref().unwrap().document_revision;
        desktop.submit(Command::Page {
            table: "Items".into(),
            offset: 0,
        });
        finish(&mut desktop, &context, &mut frame);

        let target = RuntimeContentsTarget {
            document_revision: revision,
            table: "Items".into(),
            column: "Score".into(),
            parent_ordinal: 0,
        };
        desktop.submit(Command::InspectCollection {
            target: target.clone(),
        });
        let first_request = desktop.generation;
        finish(&mut desktop, &context, &mut frame);
        assert_eq!(desktop.state.contents_request_id(), Some(first_request));
        assert_eq!(
            desktop.state.data.as_ref().unwrap().rows[0][1].text,
            "2 attachments"
        );

        let mut visible_metadata = false;
        for pass in 0..3 {
            let mut output = context.run_ui(
                egui::RawInput {
                    time: Some(f64::from(pass) * 0.25),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 820.0),
                    )),
                    ..Default::default()
                },
                |ui| assert!(desktop.state.show(ui).is_empty()),
            );
            visible_metadata = output
                .platform_output
                .accesskit_update
                .as_ref()
                .is_some_and(|tree| {
                    tree.nodes.iter().any(|(_, node)| {
                        node.value()
                            .is_some_and(|text| text.contains("synthetic.bin"))
                    })
                });
            output.textures_delta.clear();
        }
        assert!(visible_metadata, "collection metadata was not presented");
        desktop.close_contents(first_request);
        assert!(desktop.state.contents_request_id().is_none());

        // An old dialog close must not cancel a newer request for the same cell.
        desktop.submit(Command::InspectCollection {
            target: target.clone(),
        });
        let second_request = desktop.generation;
        desktop.close_contents(first_request);
        assert_eq!(desktop.state.contents_request_id(), Some(second_request));
        assert!(desktop.state.busy);
        desktop.close_contents(second_request);
        assert!(desktop.state.contents_request_id().is_none());
        assert!(!desktop.state.busy);

        desktop.open();
        finish(&mut desktop, &context, &mut frame);
        assert_ne!(
            desktop.state.database.as_ref().unwrap().document_revision,
            revision
        );
        let before = desktop.generation;
        desktop.submit(Command::InspectCollection { target });
        assert_eq!(desktop.generation, before);
        assert!(desktop.state.contents_request_id().is_none());
        assert!(!desktop.state.busy);
        assert!(desktop.state.error.is_some());
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}
