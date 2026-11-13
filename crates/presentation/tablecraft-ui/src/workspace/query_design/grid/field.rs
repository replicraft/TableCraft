//! Field-row authoring retains exact text until checked commit or explicit Cancel.
use super::{InputBudget, bounded_input};
use crate::localization::tr;
use crate::workspace::focus::{self, Pane};
use crate::workspace::query_design::{
    context_menu,
    field_inputs::{FieldInputs, copy_text, payload_bytes},
};
use egui::{EventFilter, Response, Ui};
use std::borrow::Cow;
use tablecraft_builders::query::{
    DesignColumn, DesignExpression, DesignSource, DesignTableMetadata, QueryDesignError,
    edit_field_operand, field_cell_complete, field_cell_text, parse_field_cell,
};

pub(super) struct Cell<'a> {
    pub sources: &'a [DesignSource],
    pub metadata: &'a [DesignTableMetadata],
    pub column: &'a mut DesignColumn,
    pub index: usize,
}

pub(super) fn show(
    ui: &mut Ui,
    cell: Cell<'_>,
    inputs: &mut FieldInputs,
    budget: &mut InputBudget,
) -> (Response, bool, bool) {
    let Cell {
        sources,
        metadata,
        column,
        index,
    } = cell;
    let reveal = inputs.reveal == Some(index);
    if reveal {
        inputs.reveal = None;
    }
    let display = inputs
        .text(index)
        .map(Cow::Borrowed)
        .map(Ok)
        .unwrap_or_else(|| field_cell_text(column));
    let Ok(display) = display else {
        budget.rejected = true;
        return (ui.label(tr(ui, "qbe-limit")), false, false);
    };
    let original = display.as_ref();
    let mut text = Cow::Borrowed(original);
    let Ok(cost) = inputs.edit_cost(index, column) else {
        budget.rejected = true;
        return (ui.label(tr(ui, "qbe-limit")), false, false);
    };
    let previous_budget = budget.remaining;
    // Display punctuation is not a change. Allow no-op replacement at the
    // boundary; a changed candidate must still pay its complete raw bytes.
    let response = bounded_input(
        ui,
        &mut text,
        "qbe-field-input",
        cost.max(original.len()),
        budget,
        // Escape must reach this editor before egui's begin-pass focus
        // navigation surrenders focus. Preserve TextEdit's arrow/Tab policy.
        Some(EventFilter {
            horizontal_arrows: true,
            vertical_arrows: true,
            escape: true,
            ..EventFilter::default()
        }),
    );
    focus::register(
        ui,
        Pane::QueryGrid,
        &response,
        index == 0 || response.has_focus(),
    );
    if reveal {
        response.request_focus();
    }
    let changed_text = (response.changed() && text.as_ref() != original).then(|| text.into_owned());
    drop(display);
    let mut changed = false;
    if let Some(text) = changed_text {
        // A current bound Field is logical query data, not a stale focus or
        // native capability. Recheck it through the owner against fresh metadata.
        // Never reconstruct a source from the explicit-Cancel baseline.
        let current_source = match &column.expression {
            DesignExpression::Field(field) => Some(field.source_id),
            DesignExpression::AllFields(source) => *source,
            DesignExpression::Expression(_) => None,
        };
        let hint = inputs.source_hint(index).or(current_source);
        let prepared = if text.len() > previous_budget.saturating_add(cost) {
            Err(QueryDesignError::Limit)
        } else {
            prepare(sources, metadata, column, &text, hint)
        };
        match prepared {
            Ok(prepared) => {
                let complete = prepared.is_some();
                let retained_hint = match prepared
                    .as_ref()
                    .and_then(|value| value.expression.as_ref())
                {
                    Some(DesignExpression::Expression(_)) => None,
                    _ => hint,
                };
                if inputs
                    .record(index, column, text, complete, retained_hint)
                    .is_ok()
                {
                    if let Some(prepared) = prepared {
                        column.alias = prepared.alias;
                        if let Some(expression) = prepared.expression {
                            column.expression = expression;
                        }
                    }
                    // Incomplete syntax is retained authoring data with its
                    // own revision; it grants no authority to the old model.
                    match inputs.edit_cost(index, column) {
                        Ok(next_cost) => {
                            budget.remaining = previous_budget
                                .saturating_add(cost)
                                .saturating_sub(next_cost)
                        }
                        Err(_) => budget.rejected = true,
                    }
                    changed = true;
                } else {
                    budget.remaining = previous_budget;
                    budget.rejected = true;
                }
            }
            Err(_) => {
                // Hard parser/allocation limits reject this entire candidate,
                // preserving the prior raw text, model and draft revision.
                budget.remaining = previous_budget;
                budget.rejected = true;
            }
        }
    } else {
        budget.remaining = previous_budget;
    }
    if response.lost_focus() {
        inputs.blur(index);
    }
    if response.has_focus()
        && !egui::Popup::is_any_open(ui.ctx())
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        if inputs.has(index) {
            match cancel(inputs, index, column, budget) {
                Ok(cancelled) => changed |= cancelled,
                Err(_) => budget.rejected = true,
            }
        } else {
            // With no retained edit, keep the ordinary Escape focus behavior.
            response.surrender_focus();
        }
    }
    let mut remove = false;
    context_menu::show(ui, &response, |ui, keyboard_open| {
        if inputs.has(index) && ui.button(tr(ui, "qbe-cancel-field-edit")).clicked() {
            match cancel(inputs, index, column, budget) {
                Ok(cancelled) => changed |= cancelled,
                Err(_) => budget.rejected = true,
            }
            ui.close();
        }
        // The Field row is the single authoring owner while raw text exists.
        // A second alias editor must not overwrite an unresolved operand.
        ui.add_enabled_ui(!inputs.has(index), |ui| {
            let mut alias = Cow::Borrowed(column.alias.as_deref().unwrap_or(""));
            ui.label(tr(ui, "qbe-alias"));
            let alias_input = bounded_input(
                ui,
                &mut alias,
                "qbe-alias-input",
                column.alias.as_ref().map_or(0, String::len),
                budget,
                None,
            );
            if keyboard_open && !inputs.has(index) {
                alias_input.request_focus();
            }
            if alias_input.changed() {
                column.alias = (!alias.is_empty()).then(|| alias.into_owned());
                changed = true;
            }
        })
        .response
        .on_disabled_hover_text(tr(ui, "qbe-finish-field-edit"));
        if ui.button(tr(ui, "qbe-remove-column")).clicked() {
            remove = true;
            ui.close();
        }
    });
    (response, changed, remove)
}

struct Prepared {
    alias: Option<String>,
    expression: Option<DesignExpression>,
}

fn prepare(
    sources: &[DesignSource],
    metadata: &[DesignTableMetadata],
    column: &DesignColumn,
    text: &str,
    hint: Option<u64>,
) -> Result<Option<Prepared>, QueryDesignError> {
    let (alias, operand) = match parse_field_cell(text) {
        Ok(parsed) => parsed,
        Err(QueryDesignError::Limit) => return Err(QueryDesignError::Limit),
        Err(_) => return Ok(None),
    };
    // NO_CLONE: The parsed mutable operand and retained exact authoring text
    // have independent lifetimes. The builder owns all field/source grammar.
    let proposed = edit_field_operand(sources, metadata, hint, copy_text(operand)?)?;
    let complete = match &proposed {
        DesignExpression::Field(_) | DesignExpression::AllFields(_) => !operand.is_empty(),
        DesignExpression::Expression(_) => field_cell_complete(text)?,
    };
    if !complete {
        return Ok(None);
    }
    let previous = field_cell_text(column)?;
    let (_, previous_operand) = parse_field_cell(&previous)?;
    let expression = if operand == previous_operand {
        // An alias-only edit preserves the existing semantic expression. This
        // does not restore a revoked source hint from a Cancel baseline.
        None
    } else {
        Some(proposed)
    };
    Ok(Some(Prepared { alias, expression }))
}

fn cancel(
    inputs: &mut FieldInputs,
    index: usize,
    column: &mut DesignColumn,
    budget: &mut InputBudget,
) -> Result<bool, QueryDesignError> {
    let previous = inputs.edit_cost(index, column)?;
    let cancelled = inputs.cancel(index, column)?;
    if cancelled {
        let restored = payload_bytes(column)?;
        budget.remaining = budget
            .remaining
            .saturating_add(previous)
            .saturating_sub(restored);
    }
    Ok(cancelled)
}
