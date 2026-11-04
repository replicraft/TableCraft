use crate::file_dialog::{FileDialog, SelectedFile};
use std::path::PathBuf;
use tablecraft_runtime::{Command, Response, Worker};
use tablecraft_ui::{Action, Data, Database, Field, Schema, State, Table};
use zeroize::Zeroize;

pub(super) struct Desktop {
    state: State,
    worker: Worker,
    generation: u64,
    query_pending: bool,
    file_dialog: FileDialog,
    selected_file: Option<SelectedFile>,
}

impl Desktop {
    pub(super) fn new(ctx: egui::Context, path: Option<PathBuf>) -> Self {
        let worker = Worker::new(move || ctx.request_repaint());
        let mut app = Self {
            state: State::default(),
            worker,
            generation: 0,
            query_pending: false,
            file_dialog: FileDialog::default(),
            selected_file: None,
        };
        app.state.file_picker_available = true;
        if let Some(path) = path {
            app.select_file(path);
            app.open();
        }
        app
    }

    fn submit(&mut self, command: Command) {
        if matches!(&command, Command::Describe { .. }) {
            self.state.prepare_schema_view();
        }
        self.query_pending = matches!(&command, Command::Query { .. });
        self.generation = self.worker.request(command);
        self.state.busy = true;
        self.state.error = None;
    }

    fn select_file(&mut self, path: PathBuf) {
        let selected = SelectedFile::new(path);
        self.state.path = selected.display().to_owned();
        self.selected_file = Some(selected);
    }

    fn poll_file_dialog(&mut self) {
        if let Some(Some(path)) = self.file_dialog.poll() {
            self.select_file(path);
            self.open();
        }
    }

    fn open(&mut self) {
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
        self.submit(Command::Open { path, password });
    }

    fn poll(&mut self) {
        while let Some((generation, result)) = self.worker.poll() {
            if generation != self.generation {
                continue;
            }
            self.state.busy = false;
            match result {
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
                    title,
                    format,
                    tables,
                }) => {
                    self.state.database = Some(Database {
                        title,
                        format,
                        tables: tables
                            .into_iter()
                            .map(|table| Table {
                                name: table.name,
                                system: table.system,
                                storage: table.storage,
                            })
                            .collect(),
                    });
                }
                Ok(Response::Data {
                    table,
                    offset,
                    columns,
                    rows,
                    has_more,
                }) => {
                    let data = Data {
                        table,
                        offset,
                        columns,
                        rows,
                        has_more,
                    };
                    if self.query_pending {
                        self.state.set_query_data(data);
                    } else {
                        self.state.set_table_data(data);
                    }
                }
                Err(error) => self.state.error = Some(error.to_string()),
            }
        }
    }
}

impl eframe::App for Desktop {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // eframe also calls logic for repaint requests while the viewport is
        // hidden. Task completion must not depend on drawing a visible frame.
        self.poll_file_dialog();
        self.poll();
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
                Action::ViewSchema { table } => self.submit(Command::Describe { table }),
                Action::Query => self.submit(Command::Query {
                    sql: self.state.sql.clone(),
                }),
                Action::Cancel => {
                    self.worker.cancel();
                    self.state.busy = false;
                    self.state.error = Some("Operation cancelled.".into());
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
}
