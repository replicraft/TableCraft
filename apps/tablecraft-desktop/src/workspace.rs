use std::path::PathBuf;
use tablecraft_runtime::{Command, Response, Worker};
use tablecraft_ui::{Action, Data, Database, State, Table};
use zeroize::Zeroize;

pub(super) struct Desktop {
    state: State,
    worker: Worker,
    generation: u64,
}

impl Desktop {
    pub(super) fn new(ctx: egui::Context, path: Option<PathBuf>) -> Self {
        let worker = Worker::new(move || ctx.request_repaint());
        let mut app = Self {
            state: State::default(),
            worker,
            generation: 0,
        };
        if let Some(path) = path {
            app.state.path = path.to_string_lossy().into_owned();
            app.open();
        }
        app
    }

    fn submit(&mut self, command: Command) {
        self.generation = self.worker.request(command);
        self.state.busy = true;
        self.state.error = None;
    }

    fn open(&mut self) {
        let password = std::mem::take(&mut self.state.password);
        self.state.database = None;
        self.state.data = None;
        self.submit(Command::Open {
            path: PathBuf::from(&self.state.path),
            password,
        });
    }

    fn poll(&mut self) {
        while let Some((generation, result)) = self.worker.poll() {
            if generation != self.generation {
                continue;
            }
            self.state.busy = false;
            match result {
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
                    self.state.data = Some(Data {
                        table,
                        offset,
                        columns,
                        rows,
                        has_more,
                    });
                }
                Err(error) => self.state.error = Some(error.to_string()),
            }
        }
    }
}

impl eframe::App for Desktop {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        for action in self.state.show(ui) {
            match action {
                Action::Open => self.open(),
                Action::Page { table, offset } => self.submit(Command::Page { table, offset }),
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
