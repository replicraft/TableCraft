//! Metadata-issued common filters retain entered operands without interpreting them.
mod dialog;
mod input;

use super::{Operation, State};
use crate::InputHint;
use crate::workspace::objects::{ObjectKind, ObjectTab};
use tablecraft_display::Options;

const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_OPERATORS: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldFilterTarget {
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub filter_id: Option<u64>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldFilterPatternMode {
    Ansi89,
    Ansi92,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldFilterOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Between,
    NotBetween,
    IsNull,
    IsNotNull,
    BeginsWith,
    NotBeginsWith,
    EndsWith,
    NotEndsWith,
    Contains,
    NotContains,
}

impl FieldFilterOperator {
    pub fn operand_count(self) -> usize {
        match self {
            Self::IsNull | Self::IsNotNull => 0,
            Self::Between | Self::NotBetween => 2,
            _ => 1,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Equal => "filtering-field-equal",
            Self::NotEqual => "filtering-field-not-equal",
            Self::Less => "filtering-field-less",
            Self::LessOrEqual => "filtering-field-less-equal",
            Self::Greater => "filtering-field-greater",
            Self::GreaterOrEqual => "filtering-field-greater-equal",
            Self::Between => "filtering-field-between",
            Self::NotBetween => "filtering-field-not-between",
            Self::IsNull => "filtering-is-null",
            Self::IsNotNull => "filtering-is-not-null",
            Self::BeginsWith => "filtering-field-begins",
            Self::NotBeginsWith => "filtering-field-not-begins",
            Self::EndsWith => "filtering-field-ends",
            Self::NotEndsWith => "filtering-field-not-ends",
            Self::Contains => "filtering-field-contains",
            Self::NotContains => "filtering-field-not-contains",
        }
    }
}

pub struct FieldFilterDescription {
    pub target: FieldFilterTarget,
    pub input_hint: InputHint,
    pub nullable: bool,
    pub operators: Vec<FieldFilterOperator>,
    pub pattern_mode: Option<FieldFilterPatternMode>,
}

pub(super) struct Session {
    target: FieldFilterTarget,
    object: ObjectTab,
    publication: u64,
    formatting: Options,
    request_id: u64,
    description: Option<FieldFilterDescription>,
    operator: FieldFilterOperator,
    inputs: [String; 2],
    applying: bool,
    error: Option<String>,
}

impl Session {
    pub(super) fn operator(&self) -> FieldFilterOperator {
        self.operator
    }

    fn inputs(&self) -> Vec<String> {
        self.inputs[..self.operator.operand_count()].to_vec()
    }

    fn inputs_fit(&self) -> bool {
        self.inputs[..self.operator.operand_count()]
            .iter()
            .all(|input| input.len() <= MAX_INPUT_BYTES)
    }

    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut count = crate::workspace::objects::memory::Counter::new(size_of::<Self>());
        count.string(&self.target.table);
        count.string(&self.target.column);
        if let ObjectKind::Table { name } = &self.object.target.kind {
            count.string(name);
        }
        for input in &self.inputs {
            count.string(input);
        }
        count.optional(&self.error);
        if let Some(description) = &self.description {
            count.string(&description.target.table);
            count.string(&description.target.column);
            count.vector(&description.operators);
        }
        count.finish()
    }
}

impl State {
    pub fn field_filter_dialog_open(&self) -> bool {
        self.table_filter.field.is_some()
    }

    pub fn field_filter_request_id(&self) -> Option<u64> {
        self.table_filter
            .field
            .as_ref()
            .map(|field| field.request_id)
    }

    pub(in crate::workspace) fn field_filter_target(
        &self,
        column: &str,
    ) -> Option<FieldFilterTarget> {
        let data = self.data.as_ref()?;
        let target = FieldFilterTarget {
            document_revision: data.source_revision?,
            table: data.table.clone(),
            column: column.to_owned(),
            filter_id: self.active_filter_target().map(|target| target.filter_id),
            enabled: self.table_filter_enabled(),
        };
        self.can_describe_field_filter(&target).then_some(target)
    }

    pub(in crate::workspace) fn current_field_filter_target(&self) -> Option<FieldFilterTarget> {
        if self.total_selection_active() || self.table_selection.row_selected() {
            return None;
        }
        let (_, column) = self.table_selection.position()?;
        let name = &self.data.as_ref()?.columns.get(column)?.name;
        self.field_filter_target(name)
    }

    pub fn can_describe_field_filter(&self, target: &FieldFilterTarget) -> bool {
        self.filter_controls_ready()
            && !self.find_window_open()
            && self.field_source_current(target)
            && self.active_object().is_some_and(|object| {
                object.target.document_revision == target.document_revision
                    && matches!(object.target.kind, ObjectKind::Table { name } if name == target.table)
            })
    }

    fn field_source_current(&self, target: &FieldFilterTarget) -> bool {
        self.filter_source_current(target.document_revision, &target.table)
            && self
                .data
                .as_ref()
                .is_some_and(|data| data.column_index(&target.column).is_some())
            && self.active_filter_target().map(|target| target.filter_id) == target.filter_id
            && self.table_filter_enabled() == target.enabled
    }

    fn field_witness_current(&self, field: &Session) -> bool {
        self.active_object().as_ref() == Some(&field.object)
            && self.focus_publication == field.publication
            && self.formatting == field.formatting
            && self.view == crate::workspace::DocumentView::Table
            && !self.backstage_visible()
            && !self.zoom_dialog_open()
            && self.contents_request_id().is_none()
            && !self.saved_query_prompt_open()
            && self.edit.is_none()
            && !self.insertion_dialog_open()
            && !self.deletion_dialog_open()
            && self.field_source_current(&field.target)
    }

    pub fn begin_field_filter(&mut self, target: FieldFilterTarget, request_id: u64) -> bool {
        if !self.can_describe_field_filter(&target) {
            return false;
        }
        let object = self.active_object().expect("admitted native table object");
        self.table_filter.field = Some(Box::new(Session {
            target,
            object,
            publication: self.focus_publication,
            formatting: self.formatting.clone(),
            request_id,
            description: None,
            operator: FieldFilterOperator::Equal,
            inputs: [String::new(), String::new()],
            applying: false,
            error: None,
        }));
        true
    }

    pub fn set_field_filter_description(
        &mut self,
        request_id: u64,
        description: FieldFilterDescription,
    ) -> bool {
        let Some(field) = &self.table_filter.field else {
            return false;
        };
        let operators = &description.operators;
        let pattern_shape =
            (description.input_hint == InputHint::Text) == description.pattern_mode.is_some();
        let valid = request_id == field.request_id
            && field.description.is_none()
            && field.error.is_none()
            && description.target == field.target
            && pattern_shape
            && !operators.is_empty()
            && operators.len() <= MAX_OPERATORS
            && operators
                .iter()
                .enumerate()
                .all(|(index, operator)| !operators[..index].contains(operator))
            && self.field_witness_current(field);
        if !valid {
            return false;
        }
        let field = self
            .table_filter
            .field
            .as_mut()
            .expect("matched metadata request");
        field.operator = description.operators[0];
        field.description = Some(description);
        field.error = None;
        true
    }

    pub fn can_apply_field_filter(
        &self,
        target: &FieldFilterTarget,
        operator: FieldFilterOperator,
        inputs: &[String],
    ) -> bool {
        let Some(field) = &self.table_filter.field else {
            return false;
        };
        !self.busy
            && !field.applying
            && &field.target == target
            && field.operator == operator
            && field
                .description
                .as_ref()
                .is_some_and(|description| description.operators.contains(&operator))
            && inputs.len() == operator.operand_count()
            && inputs == &field.inputs[..operator.operand_count()]
            && field.inputs_fit()
            && self.field_witness_current(field)
    }

    pub fn begin_field_filter_apply(
        &mut self,
        request_id: u64,
        target: FieldFilterTarget,
        operator: FieldFilterOperator,
        inputs: Vec<String>,
    ) -> bool {
        if !self.can_apply_field_filter(&target, operator, &inputs) {
            return false;
        }
        let field = self
            .table_filter
            .field
            .as_mut()
            .expect("admitted field draft");
        field.request_id = request_id;
        field.applying = true;
        field.error = None;
        let column = target.column.clone();
        self.begin_filter_operation(request_id, Operation::Field { target, operator });
        if let Some(pending) = &mut self.table_filter.pending {
            pending.column = Some(column);
        }
        true
    }

    pub(super) fn field_reply_current(&self, request_id: u64) -> bool {
        self.table_filter.field.as_ref().is_some_and(|field| {
            field.request_id == request_id && field.applying && self.field_witness_current(field)
        })
    }

    pub fn fail_field_filter(&mut self, request_id: u64, message: String) {
        let accepts = self.table_filter.field.as_ref().is_some_and(|field| {
            field.request_id == request_id
                && field.error.is_none()
                && self.field_witness_current(field)
        });
        if accepts {
            self.table_filter.pending = None;
            let field = self
                .table_filter
                .field
                .as_mut()
                .expect("matched field request");
            field.applying = false;
            field.error = Some(message);
        }
    }

    pub fn close_field_filter(&mut self, request_id: u64) -> bool {
        if self.field_filter_request_id() != Some(request_id) {
            return false;
        }
        self.table_filter.field = None;
        if self
            .table_filter
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            self.table_filter.pending = None;
        }
        true
    }

    pub(in crate::workspace) fn validate_field_filter(&mut self) {
        if self
            .table_filter
            .field
            .as_ref()
            .is_some_and(|field| !self.field_witness_current(field))
        {
            let request = self
                .field_filter_request_id()
                .expect("existing field request");
            self.close_field_filter(request);
        }
    }
}

#[cfg(test)]
mod tests;
