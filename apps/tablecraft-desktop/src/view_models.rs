//! Move owned runtime cell facts into the renderer-neutral presentation model.
use tablecraft_runtime::{
    Cell as RuntimeCell, CellKind as RuntimeKind, CollectionKind as RuntimeCollectionKind,
    Column as RuntimeColumn, ContentsTarget as RuntimeContentsTarget,
    DisplayControl as RuntimeDisplayControl, Field as RuntimeField,
    FieldPropertyValue as RuntimeFieldPropertyValue, HorizontalAlignment as RuntimeAlignment,
    InputHint as RuntimeHint,
};
use tablecraft_ui::{
    Cell, CellKind, CollectionKind, Column, ContentsTarget, DisplayControl, Field, FieldProperty,
    FieldPropertyValue, HorizontalAlignment, InputHint,
};

pub(super) fn fields(fields: Vec<RuntimeField>) -> Vec<Field> {
    fields
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
            display_type: None,
            field_size: None,
            reader_nullable: field.reader_nullable,
            decimal_parameters: field.decimal_parameters,
            complex_type_id: field.complex_type_id,
            properties: field
                .properties
                .into_iter()
                .map(|property| FieldProperty {
                    name: property.name,
                    value: match property.value {
                        RuntimeFieldPropertyValue::Uninspected => FieldPropertyValue::Uninspected,
                        RuntimeFieldPropertyValue::Absent => FieldPropertyValue::Absent,
                        RuntimeFieldPropertyValue::Boolean(value) => {
                            FieldPropertyValue::Boolean(value)
                        }
                        RuntimeFieldPropertyValue::Text(value) => FieldPropertyValue::Text(value),
                        RuntimeFieldPropertyValue::Unsupported => FieldPropertyValue::Unsupported,
                    },
                })
                .collect(),
        })
        .collect()
}

pub(super) fn columns(columns: Vec<RuntimeColumn>) -> Vec<Column> {
    columns
        .into_iter()
        .map(|column| Column {
            name: column.name,
            caption: column.caption,
            native_number: column.native_number,
        })
        .collect()
}

pub(super) fn rows(rows: Vec<Vec<RuntimeCell>>) -> Vec<Vec<Cell>> {
    rows.into_iter()
        .map(|row| row.into_iter().map(cell).collect())
        .collect()
}

pub(super) fn contents_target(target: &RuntimeContentsTarget) -> ContentsTarget {
    ContentsTarget {
        document_revision: target.document_revision,
        table: target.table.clone(),
        column: target.column.clone(),
        parent_ordinal: target.parent_ordinal,
    }
}

pub(super) fn collection_kind(kind: RuntimeCollectionKind) -> CollectionKind {
    match kind {
        RuntimeCollectionKind::Multivalue => CollectionKind::Multivalue,
        RuntimeCollectionKind::Attachments => CollectionKind::Attachments,
        RuntimeCollectionKind::VersionHistory => CollectionKind::VersionHistory,
    }
}

pub(super) fn cell(cell: RuntimeCell) -> Cell {
    let kind = match cell.kind {
        RuntimeKind::Scalar {
            is_null,
            stored_calculation,
            boolean,
            display_control,
        } => CellKind::Scalar {
            is_null,
            stored_calculation,
            boolean,
            display_control: match display_control {
                RuntimeDisplayControl::Uninspected => DisplayControl::Uninspected,
                RuntimeDisplayControl::Absent => DisplayControl::Absent,
                RuntimeDisplayControl::Native(code) => DisplayControl::Native(code),
                RuntimeDisplayControl::Unsupported => DisplayControl::Unsupported,
            },
        },
        RuntimeKind::Multivalue { assigned, members } => CellKind::Multivalue { assigned, members },
        RuntimeKind::Attachments { assigned, members } => {
            CellKind::Attachments { assigned, members }
        }
        RuntimeKind::VersionHistory { assigned, members } => {
            CellKind::VersionHistory { assigned, members }
        }
    };
    Cell {
        presentation: cell.presentation,
        alignment: match cell.alignment {
            RuntimeAlignment::Left => HorizontalAlignment::Left,
            RuntimeAlignment::Right => HorizontalAlignment::Right,
        },
        text: cell.text,
        kind,
    }
}

/// Mirror instruction tags only; the host never selects translations or parsers.
pub(super) fn input_hint(value: RuntimeHint) -> InputHint {
    match value {
        RuntimeHint::Boolean => InputHint::Boolean,
        RuntimeHint::Byte => InputHint::Byte,
        RuntimeHint::Int16 => InputHint::Int16,
        RuntimeHint::Int32 => InputHint::Int32,
        RuntimeHint::Int64 => InputHint::Int64,
        RuntimeHint::Single => InputHint::Single,
        RuntimeHint::Double => InputHint::Double,
        RuntimeHint::Currency => InputHint::Currency,
        RuntimeHint::Decimal => InputHint::Decimal,
        RuntimeHint::Text => InputHint::Text,
        RuntimeHint::BinaryHex => InputHint::BinaryHex,
        RuntimeHint::Guid => InputHint::Guid,
        RuntimeHint::DateTimeIso => InputHint::DateTimeIso,
        RuntimeHint::DateTimeExtendedIso => InputHint::DateTimeExtendedIso,
    }
}

#[cfg(test)]
mod tests {
    use super::input_hint;
    use tablecraft_runtime::InputHint as RuntimeHint;
    use tablecraft_ui::InputHint;

    #[test]
    fn every_runtime_input_hint_crosses_the_host_without_selecting_prose_or_parsers() {
        for (runtime, expected) in [
            (RuntimeHint::Boolean, InputHint::Boolean),
            (RuntimeHint::Byte, InputHint::Byte),
            (RuntimeHint::Int16, InputHint::Int16),
            (RuntimeHint::Int32, InputHint::Int32),
            (RuntimeHint::Int64, InputHint::Int64),
            (RuntimeHint::Single, InputHint::Single),
            (RuntimeHint::Double, InputHint::Double),
            (RuntimeHint::Currency, InputHint::Currency),
            (RuntimeHint::Decimal, InputHint::Decimal),
            (RuntimeHint::Text, InputHint::Text),
            (RuntimeHint::BinaryHex, InputHint::BinaryHex),
            (RuntimeHint::Guid, InputHint::Guid),
            (RuntimeHint::DateTimeIso, InputHint::DateTimeIso),
            (
                RuntimeHint::DateTimeExtendedIso,
                InputHint::DateTimeExtendedIso,
            ),
        ] {
            assert_eq!(input_hint(runtime), expected);
        }
    }
}
