//! Retain exact insertion intent across schema publication and native save outcomes.
use super::{Dialog, InsertRetryPermission, retained::RetainedInput};
use crate::localization::tr;
use crate::workspace::objects::memory::Counter;
use crate::workspace::{
    Action, Database, InitialRecord, InsertInput, InsertTarget, ObjectKind, ObjectTarget, State,
};
use egui::{Context, Id, Modal};
use tablecraft_builders::table::DraftFieldType;

const MAX_FAILURE_BYTES: usize = 4096;
const MODAL_OUTER_MARGIN: f32 = 48.0;
const MODAL_MIN_WIDTH: f32 = 240.0;
const MODAL_MAX_WIDTH: f32 = 600.0;
const COPY_BODY_HEIGHT: f32 = 260.0;
const COPY_BODY_RESERVE: f32 = 180.0;
const MIN_COPY_BODY_HEIGHT: f32 = 48.0;

pub(super) enum Seed {
    Initial(InitialRecord),
    Retained(RetainedInput),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RetryPhase {
    Ready,
    Reopening {
        operation_id: u64,
        cancel_requested: bool,
    },
    Qualified,
}

struct Retry {
    save_id: u64,
    permission: InsertRetryPermission,
    phase: RetryPhase,
    outcome: String,
    message: Option<String>,
}

pub(in crate::workspace) struct Continuation {
    target: InsertTarget,
    pub(super) source: Seed,
    request: Option<u64>,
    auto_start: bool,
    error: bool,
    retry: Option<Retry>,
}

impl Continuation {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        count.string(&self.target.table);
        count.add(match &self.source {
            Seed::Initial(record) => record.retained_bytes()?,
            Seed::Retained(input) => input.retained_bytes()?,
        });
        if let Some(retry) = &self.retry {
            count.string(&retry.outcome);
            count.optional(&retry.message);
        }
        count.finish()
    }

    pub(super) fn retained(
        save_id: u64,
        permission: InsertRetryPermission,
        input: RetainedInput,
        outcome: String,
    ) -> Self {
        Self {
            // the retained command or exact-owner witness needs its own captured identity after
            // this render/dispatch; keep the original catalog, input or owner available for the
            // later checks in this operation.
            target: input.draft.target.clone(),
            source: Seed::Retained(input),
            request: None,
            auto_start: false,
            error: false,
            retry: Some(Retry {
                save_id,
                permission,
                phase: RetryPhase::Ready,
                outcome: bounded_message(&outcome),
                message: None,
            }),
        }
    }
}

impl State {
    /// Prepare before discarding any schema editor. Failure returns all input.
    pub(in crate::workspace) fn prepare_initial_record(
        &self,
        target: &ObjectTarget,
        record: InitialRecord,
    ) -> Result<Continuation, InitialRecord> {
        let ObjectKind::Table { name } = &target.kind else {
            return Err(record);
        };
        if self.insert_continuation.is_some()
            || name != record.snapshot.table_name()
            || record.inputs.len() != record.snapshot.fields().len()
            || record.raw_text.len() != record.inputs.len()
            || record.inputs.is_empty()
        {
            return Err(record);
        }
        for (position, ((field, input), raw)) in record
            .snapshot
            .fields()
            .iter()
            .zip(&record.inputs)
            .zip(&record.raw_text)
            .enumerate()
        {
            let generated = field.kind() == DraftFieldType::AutoNumberIncrement;
            if field.is_protected() != generated
                || generated
                    && (!field.is_primary()
                        || position != 0
                        || !matches!(input, InsertInput::Omitted)
                        || !raw.is_empty())
                || matches!(input, InsertInput::Value(value) if &value.text != raw)
            {
                return Err(record);
            }
        }
        Ok(Continuation {
            target: InsertTarget {
                document_revision: target.document_revision,
                // InsertTarget needs its own captured identity after this render/dispatch; keep
                // the original catalog, input or owner available for the later checks in this
                // operation.
                table: name.clone(),
            },
            source: Seed::Initial(record),
            request: None,
            auto_start: true,
            error: false,
            retry: None,
        })
    }

    pub fn insert_continuation_pending(&self) -> bool {
        self.insert_continuation.is_some()
    }

    pub(super) fn insert_continuation_allows(&self, target: &InsertTarget) -> bool {
        self.insert_continuation.as_ref().is_none_or(|owner| {
            &owner.target == target
                && owner
                    .retry
                    .as_ref()
                    .is_none_or(|retry| retry.phase == RetryPhase::Qualified)
        })
    }

    pub fn can_retry_insert(&self, save_id: u64) -> bool {
        !self.busy
            && self.insertion.is_none()
            && self.insert_continuation.as_ref().is_some_and(|owner| {
                owner.retry.as_ref().is_some_and(|retry| {
                    retry.save_id == save_id
                        && retry.permission == InsertRetryPermission::OriginalImage
                        && retry.phase == RetryPhase::Ready
                })
            })
    }

    pub fn begin_insert_retry(&mut self, save_id: u64, operation_id: u64) -> bool {
        if !self.can_retry_insert(save_id) {
            return false;
        }
        let owner = self.insert_continuation.as_mut().unwrap();
        let retry = owner.retry.as_mut().unwrap();
        retry.phase = RetryPhase::Reopening {
            operation_id,
            cancel_requested: false,
        };
        retry.message = None;
        owner.error = false;
        true
    }

    /// Publish only metadata from the host's completed original-image guarded read.
    pub fn complete_insert_retry(
        &mut self,
        operation_id: u64,
        database: Database,
    ) -> Option<ObjectTarget> {
        let owner = self.insert_continuation.as_ref()?;
        let retry = owner.retry.as_ref()?;
        if !matches!(retry.phase, RetryPhase::Reopening { operation_id: current, .. } if current == operation_id)
            || database.document_revision == owner.target.document_revision
            || !database
                .tables
                .iter()
                .any(|table| table.name == owner.target.table && table.local && !table.system)
        {
            return None;
        }
        let target = ObjectTarget {
            document_revision: database.document_revision,
            kind: ObjectKind::Table {
                // ObjectTarget needs its own captured identity after this render/dispatch; keep
                // the original catalog, input or owner available for the later checks in this
                // operation.
                name: owner.target.table.clone(),
            },
        };
        let owner = self.insert_continuation.as_mut().unwrap();
        owner.target.document_revision = database.document_revision;
        owner.retry.as_mut().unwrap().phase = RetryPhase::Qualified;
        owner.auto_start = true;
        owner.request = None;
        self.clear_object_slots();
        self.database = Some(database);
        self.synchronize_objects();
        self.dismiss_backstage();
        Some(target)
    }

    pub fn fail_insert_retry(&mut self, operation_id: u64, message: String) -> bool {
        let Some(owner) = &mut self.insert_continuation else {
            return false;
        };
        let Some(retry) = &mut owner.retry else {
            return false;
        };
        if !matches!(retry.phase, RetryPhase::Reopening { operation_id: current, .. } if current == operation_id)
        {
            return false;
        }
        let mut end = message.len().min(MAX_FAILURE_BYTES);
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        retry.message = Some(message[..end].into());
        retry.phase = RetryPhase::Ready;
        owner.auto_start = false;
        owner.error = true;
        true
    }

    pub fn discard_retained_insert(&mut self, save_id: u64) -> bool {
        if self.busy
            || self.insertion.is_some()
            || !self.insert_continuation.as_ref().is_some_and(|owner| {
                owner.retry.as_ref().is_some_and(|retry| {
                    retry.save_id == save_id && retry.phase == RetryPhase::Ready
                })
            })
        {
            return false;
        }
        self.insert_continuation = None;
        true
    }

    pub(in crate::workspace) fn insert_continuation_started(&mut self, request_id: u64) {
        let Some(owner) = &mut self.insert_continuation else {
            return;
        };
        if let Some(Dialog::Checking { target, .. }) = &self.insertion
            && *target == owner.target
        {
            owner.request = Some(request_id);
            owner.auto_start = false;
        }
    }

    pub(in crate::workspace) fn adopt_insert_continuation(
        &mut self,
        request_id: u64,
        draft_id: u64,
    ) {
        let Some(mut owner) = self.insert_continuation.take() else {
            return;
        };
        if owner.request != Some(request_id) {
            self.insert_continuation = Some(owner);
            return;
        }
        let seeded = match &owner.source {
            Seed::Initial(record) => self.seed_initial_insert(draft_id, record),
            Seed::Retained(retained) => self.seed_retained_insert(draft_id, retained),
        };
        if seeded {
            self.initial_record_notice = match &owner.source {
                Seed::Initial(_) => Some(owner.target),
                Seed::Retained(retained) => retained.initial_notice.as_ref().map(|_| owner.target),
            };
        } else {
            owner.request = None;
            owner.auto_start = false;
            owner.error = true;
            if let Some(retry) = &mut owner.retry {
                retry.phase = RetryPhase::Ready;
            }
            self.insertion = None;
            self.insert_continuation = Some(owner);
        }
    }

    pub(in crate::workspace) fn reject_insert_continuation_draft(&mut self, request_id: u64) {
        if let Some(owner) = &mut self.insert_continuation
            && owner.request == Some(request_id)
        {
            owner.request = None;
            owner.auto_start = false;
            owner.error = true;
            if let Some(retry) = &mut owner.retry {
                retry.phase = RetryPhase::Ready;
            }
            self.insertion = None;
        }
    }

    pub(in crate::workspace) fn insert_continuation_closed(&mut self) {
        self.initial_record_notice = None;
        if let Some(owner) = &mut self.insert_continuation {
            owner.request = None;
            owner.auto_start = false;
            owner.error = true;
            if let Some(retry) = &mut owner.retry {
                retry.phase = RetryPhase::Ready;
            }
        }
    }

    pub(in crate::workspace) fn insert_continuation_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        if self.insertion.is_some() {
            return;
        }
        let Some(owner) = &self.insert_continuation else {
            return;
        };
        // the retained command or exact-owner witness needs its own captured identity after
        // this render/dispatch; keep the original catalog, input or owner available for the
        // later checks in this operation.
        let target = owner.target.clone();
        let ready = self.can_insert_row(&target);
        if owner.auto_start && ready {
            self.insert_continuation.as_mut().unwrap().auto_start = false;
            actions.push(Action::BeginInsert { target });
            return;
        }
        if owner
            .retry
            .as_ref()
            .is_some_and(|retry| retry.phase != RetryPhase::Qualified)
        {
            self.retained_insert_dialog(context, actions);
            return;
        }
        let bound = self.database.as_ref().is_some_and(|database| {
            database.document_revision == target.document_revision
                && database
                    .tables
                    .iter()
                    .any(|table| table.name == target.table && !table.system)
        });
        let failed = owner.error || !bound;
        let mut retry = false;
        let mut discard = false;
        Modal::new(Id::new("insert_continuation")).show(context, |ui| {
            ui.set_width(
                (context.content_rect().width() - MODAL_OUTER_MARGIN)
                    .clamp(MODAL_MIN_WIDTH, MODAL_MAX_WIDTH),
            );
            ui.heading(tr(ui, "insertion-title"));
            let initial_schema = matches!(&owner.source, Seed::Initial(_))
                || matches!(&owner.source, Seed::Retained(input) if input.initial_notice.is_some());
            ui.label(tr(
                ui,
                if initial_schema {
                    "table-draft-record-schema-saved"
                } else {
                    "insertion-retained-help"
                },
            ));
            if self.busy {
                ui.spinner();
                ui.label(tr(ui, "table-record-preparing"));
            } else if failed {
                ui.colored_label(ui.visuals().error_fg_color, tr(ui, "table-record-invalid"));
            }
            ui.horizontal(|ui| {
                retry = ui
                    .add_enabled(
                        !self.busy && bound,
                        egui::Button::new(tr(ui, "table-record-retry")),
                    )
                    .clicked();
                discard = ui
                    .add_enabled(
                        !self.busy,
                        egui::Button::new(tr(ui, "table-record-discard")),
                    )
                    .clicked();
            });
        });
        if discard {
            if let Some(retry) = self
                .insert_continuation
                .as_ref()
                .and_then(|owner| owner.retry.as_ref())
            {
                actions.push(Action::DiscardRetainedInsert {
                    save_id: retry.save_id,
                });
            } else {
                self.insert_continuation = None;
            }
        } else if retry {
            let owner = self.insert_continuation.as_mut().unwrap();
            owner.auto_start = true;
            owner.error = false;
            if !ready {
                actions.push(Action::Page {
                    table: target.table,
                    offset: 0,
                });
            }
        }
    }

    fn retained_insert_dialog(&mut self, context: &Context, actions: &mut Vec<Action>) {
        let owner = self.insert_continuation.as_ref().unwrap();
        let retry = owner.retry.as_ref().unwrap();
        let save_id = retry.save_id;
        let can_retry = self.can_retry_insert(save_id);
        let mut cancel = false;
        let response = Modal::new(Id::new("retained_insert_input")).show(context, |ui| {
            ui.set_width(
                (context.content_rect().width() - MODAL_OUTER_MARGIN)
                    .clamp(MODAL_MIN_WIDTH, MODAL_MAX_WIDTH),
            );
            ui.heading(tr(ui, "insertion-title"));
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            let body_height = (context.content_rect().height() - COPY_BODY_RESERVE)
                .clamp(MIN_COPY_BODY_HEIGHT, COPY_BODY_HEIGHT);
            egui::ScrollArea::vertical()
                .id_salt("retained_insert_copy")
                .max_height(body_height)
                .show(ui, |ui| {
                    ui.label(&retry.outcome);
                    if let Some(message) = &retry.message {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                    }
                    if let Seed::Retained(retained) = &owner.source {
                        retained.show(ui);
                    }
                });
            if let RetryPhase::Reopening {
                operation_id: _,
                cancel_requested,
            } = retry.phase
            {
                ui.spinner();
                ui.label(tr(ui, "insertion-reopening"));
                cancel = ui
                    .add_enabled(!cancel_requested, egui::Button::new(tr(ui, "shell-cancel")))
                    .clicked();
            } else {
                if retry.permission == InsertRetryPermission::Blocked {
                    ui.label(tr(ui, "insertion-retry-blocked"));
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            can_retry,
                            egui::Button::new(tr(ui, "insertion-retry-original")),
                        )
                        .clicked()
                    {
                        actions.push(Action::RetryInsert { save_id });
                    }
                    if ui
                        .add_enabled(
                            !self.busy,
                            egui::Button::new(tr(ui, "insertion-discard-retained")),
                        )
                        .clicked()
                    {
                        actions.push(Action::DiscardRetainedInsert { save_id });
                    }
                });
            }
        });
        cancel |= response.should_close();
        if cancel {
            let retry = self
                .insert_continuation
                .as_mut()
                .unwrap()
                .retry
                .as_mut()
                .unwrap();
            if let RetryPhase::Reopening {
                operation_id,
                cancel_requested,
            } = &mut retry.phase
                && !*cancel_requested
            {
                *cancel_requested = true;
                actions.push(Action::CancelInsertRetry {
                    operation_id: *operation_id,
                });
            }
        }
    }
}

fn bounded_message(message: &str) -> String {
    let mut end = message.len().min(MAX_FAILURE_BYTES);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].into()
}

#[cfg(test)]
mod tests;
