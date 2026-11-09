//! Structured inert output; a serialized review never becomes native authority.
use super::{Session, reviewed::Reviewed};
use serde_json::{Value, json};
use tablecraft_runtime::{Cell, CellKind, Column, FieldPropertyValue, ParameterSpec, Response};

pub(super) fn columns(values: &[Column]) -> Value {
    values.iter().map(|column| json!({"name": column.name, "caption": column.caption, "native_number": column.native_number})).collect()
}

pub(super) fn cell(value: &Cell) -> Value {
    let semantics = match value.kind {
        CellKind::Scalar {
            is_null,
            stored_calculation,
            boolean,
            display_control,
        } => {
            json!({"kind": "scalar", "is_null": is_null, "stored_calculation": stored_calculation, "boolean": boolean, "display_control": format!("{display_control:?}")})
        }
        CellKind::Multivalue { assigned, members } => {
            json!({"kind": "multivalue", "assigned": assigned, "members": members})
        }
        CellKind::Attachments { assigned, members } => {
            json!({"kind": "attachments", "assigned": assigned, "members": members})
        }
        CellKind::VersionHistory { assigned, members } => {
            json!({"kind": "version_history", "assigned": assigned, "members": members})
        }
    };
    json!({"text": value.text, "semantics": semantics})
}

pub(super) fn rows(values: &[Vec<Cell>]) -> Value {
    values
        .iter()
        .map(|row| row.iter().map(cell).collect::<Value>())
        .collect()
}

fn parameters(values: &[ParameterSpec]) -> Value {
    values.iter().map(|parameter| json!({"name": parameter.name, "kind": parameter.kind, "label": parameter.label, "nullable": parameter.nullable})).collect()
}

impl Session {
    pub(super) fn response(&mut self, response: Response) -> Result<Value, String> {
        let value = match response {
            Response::Opened { document_revision, source_path, table_creation_available, table_design_available, title, format, tables, queries } => {
                self.revision = Some(document_revision);
                self.path = Some(source_path);
                self.query = None;
                self.drop_source = None;
                self.find_state = super::auxiliary::FindState::default();
                self.design_state = super::query_design::DesignState::default();
                if self.active_save.is_some() {
                    self.view_state.retire_native();
                } else {
                    self.view_state = super::filtering::ViewState::default();
                }
                self.reviewed = None;
                self.insert_retry = None;
                self.catalog = json!({
                    "document_revision": document_revision, "title": title, "format": format,
                    "table_creation_available": table_creation_available, "table_design_available": table_design_available,
                    "tables": tables.into_iter().map(|table| json!({"name": table.name, "system": table.system, "local": table.local, "storage": table.storage})).collect::<Vec<_>>(),
                    "queries": queries.into_iter().map(|query| json!({"object_id": query.object_id.to_string(), "name": query.name, "system": query.system})).collect::<Vec<_>>()
                });
                return Ok(json!({"catalog": &self.catalog}));
            }
            Response::Data { source_revision, table, offset, columns: fields, rows: values, has_more } => json!({"document_revision": source_revision, "table": table, "offset": offset, "columns": columns(&fields), "rows": rows(&values), "has_more": has_more}),
            Response::Details { table, declared_rows, fields } => json!({"table": table, "declared_rows": declared_rows, "fields": fields.into_iter().map(|field| {
                json!({"name": field.name, "native_number": field.number, "native_type": field.native_type, "storage_bytes": field.storage_bytes, "fixed": field.fixed, "flags": field.flags, "extended_flags": field.extended_flags, "scalar_type": field.scalar_type, "reader_nullable": field.reader_nullable, "decimal_parameters": field.decimal_parameters, "complex_type_id": field.complex_type_id,
                    "properties": field.properties.into_iter().map(|property| {
                        let value = match property.value {
                            FieldPropertyValue::Boolean(value) => json!({"kind": "boolean", "value": value}),
                            FieldPropertyValue::Text(value) => json!({"kind": "text", "value": value}),
                            value => json!({"kind": format!("{value:?}")}),
                        };
                        json!({"name": property.name, "observation": value})
                    }).collect::<Vec<_>>()})
            }).collect::<Vec<_>>()}),
            Response::Contents { target, kind, assigned, columns: fields, rows: values } => json!({"document_revision": target.document_revision, "table": target.table, "column": target.column, "parent_ordinal": target.parent_ordinal, "kind": format!("{kind:?}"), "assigned": assigned, "columns": columns(&fields), "rows": rows(&values)}),
            Response::QueryParameters { target, parameters: values } => {
                let id = self.next_handle()?;
                let result = json!({"query_id": id, "document_revision": target.document_revision(), "sql": target.sql(), "parameters": parameters(&values)});
                self.query = Some((id, target));
                result
            }
            Response::SavedQuery { target, parameters: values } => json!({"document_revision": target.document_revision, "object_id": target.object_id.to_string(), "name": target.name, "parameters": parameters(&values)}),
            Response::EditDraft(draft) => json!({"draft_id": draft.id, "document_revision": draft.target.document_revision, "table": draft.target.table, "column": draft.target.column, "parent_ordinal": draft.target.parent_ordinal, "original": cell(&draft.original), "initial": {"text": draft.initial.text, "is_null": draft.initial.is_null}, "input_hint": format!("{:?}", draft.input_hint), "nullable": draft.nullable}),
            Response::EditReviewed { review, capability } => return self.retain_review(Reviewed::Edit(capability), json!({"native_review_id": review.id, "document_revision": review.target.document_revision, "table": review.target.table, "column": review.target.column, "parent_ordinal": review.target.parent_ordinal, "before": cell(&review.before), "after": cell(&review.after), "changed_rows": review.changed_rows})),
            Response::InsertDraft(draft) => json!({"draft_id": draft.id, "document_revision": draft.target.document_revision, "table": draft.target.table, "fields": draft.fields.into_iter().map(|field| json!({"name": field.name, "input_hint": format!("{:?}", field.input_hint), "nullable": field.nullable, "auto_number": field.auto_number})).collect::<Vec<_>>()}),
            Response::InsertReviewed { review, capability } => return self.retain_review(Reviewed::Insert(capability), json!({"native_review_id": review.id, "document_revision": review.target.document_revision, "table": review.target.table, "draft_id": review.draft_id, "inserted_rows": review.inserted_rows})),
            Response::DeleteReviewed { review, capability } => return self.retain_review(Reviewed::Delete(capability), json!({"native_review_id": review.id, "document_revision": review.target.document_revision, "table": review.target.table, "parent_ordinal": review.target.parent_ordinal, "deleted_rows": review.deleted_rows, "columns": columns(&review.columns), "row": review.row.iter().map(cell).collect::<Vec<_>>()})),
            Response::TableRenameReviewed { review, capability } => return self.retain_review(Reviewed::TableRename(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "table": review.table, "new_name": review.new_name, "changed": review.changed})),
            Response::TableCreationReviewed { capability } => {
                let witness = capability.reopen_witness();
                let facts = json!({"native_review_id": capability.id(), "document_revision": witness.document_revision(), "table": witness.table_name(), "columns": witness.column_count(), "draft_revision": witness.draft_revision()});
                return self.retain_review(Reviewed::TableCreation(capability), facts);
            }
            Response::TableDropSourceInspected { inspection } => {
                let result = json!({"document_revision": inspection.document_revision, "table": inspection.table, "native_id": inspection.native_id, "declared_rows": inspection.declared_rows, "source_eligible": inspection.source_eligible});
                self.drop_source = Some(inspection);
                result
            }
            Response::TableDropReviewed { review, capability } => {
                if !self.drop_source.as_ref().is_some_and(|source| source.document_revision == review.document_revision && source.table == review.table && source.native_id == review.native_id && source.declared_rows == review.declared_rows && source.source_eligible)
                    || capability.captured_document_revision() != review.document_revision
                    || capability.captured_table() != review.table
                    || capability.captured_native_id() != review.native_id
                {
                    return Err("Table DROP review differs from the inspected source".into());
                }
                self.drop_source = None;
                return self.retain_review(Reviewed::TableDrop(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "table": review.table, "native_id": review.native_id, "declared_rows": review.declared_rows}));
            }
            Response::FieldAdditionAvailability { availability } => json!({"document_revision": availability.document_revision, "table": availability.table, "number_available": availability.number_available, "short_text_available": availability.short_text_available}),
            Response::FieldAdditionReviewed { review, capability } => return self.retain_review(Reviewed::FieldAddition(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "table": review.table, "column": review.column, "declaration": format!("{:?}", review.declaration)})),
            Response::FieldRenameSourceInspected { inspection } => json!({"document_revision": inspection.document_revision, "table": inspection.table, "column": inspection.column, "native_number": inspection.native_number, "source_eligible": inspection.source_eligible, "same_width_ascii_utf16_units": inspection.same_width_ascii_utf16_units}),
            Response::FieldRenameReviewed { review, capability } => return self.retain_review(Reviewed::FieldRename(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "table": review.table, "column": review.column, "native_number": review.native_number, "new_name": review.new_name, "changed": review.changed})),
            Response::FieldDropSourceInspected { inspection } => json!({"document_revision": inspection.document_revision, "table": inspection.table, "column": inspection.column, "native_number": inspection.native_number, "source_eligible": inspection.source_eligible}),
            Response::FieldDropReviewed { review, capability } => return self.retain_review(Reviewed::FieldDrop(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "table": review.table, "column": review.column, "native_number": review.native_number})),
            Response::DatabaseOptionsInspected { inspection } => json!({"document_revision": inspection.document_revision, "name_auto_correct": format!("{:?}", inspection.name_auto_correct), "disable_available": inspection.disable_available}),
            Response::DatabaseOptionsReviewed { review, capability } => return self.retain_review(Reviewed::DatabaseOptions(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "change": format!("{:?}", review.change)})),
            Response::QuerySqlDraft { draft } => json!({"document_revision": draft.target.document_revision, "object_id": draft.target.object_id.to_string(), "name": draft.target.name, "sql": draft.sql, "eligibility": format!("{:?}", draft.eligibility)}),
            Response::QuerySqlReviewed { review, capability } => return self.retain_review(Reviewed::QuerySql(capability), json!({"native_review_id": review.id, "document_revision": review.target.document_revision, "object_id": review.target.object_id.to_string(), "name": review.target.name, "native_id": review.native_id, "changed": review.changed, "original_sql": review.original_sql, "proposed_sql": review.proposed_sql})),
            Response::QueryExpressionReviewed { review, capability } => return self.retain_review(Reviewed::QueryExpression(capability), json!({"native_review_id": review.id, "document_revision": review.target.document_revision, "native_id": review.native_id, "expected": review.expected, "replacement": review.replacement, "changed": review.changed})),
            Response::QueryInsertReviewed { review, capability } => return self.retain_review(Reviewed::QueryInsert(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "table": review.table, "inserted_rows": review.inserted_rows})),
            Response::QueryDeleteReviewed { review, capability } => return self.retain_review(Reviewed::QueryDelete(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "table": review.table, "matched_rows": review.matched_rows, "deleted_rows": review.deleted_rows})),
            Response::QueryUpdateReviewed { review, capability } => return self.retain_review(Reviewed::QueryUpdate(capability), json!({"native_review_id": review.id, "document_revision": review.document_revision, "table": review.table, "matched_rows": review.matched_rows, "changed_rows": review.changed_rows})),
            _ => return Err("The runtime returned a response outside this operation's protocol contract".into()),
        };
        Ok(value)
    }
}
