//! Qualified native Boolean Format labels under the English presentation policy.
//!
//! This is display/copy text only. Native values, edit input and query execution
//! retain their original typed carriers. No locale is inferred from the database
//! LCID or host environment, and custom formats are not evaluated.
use crate::{Cell, CellKind, DisplayControl, Error, display::Presentation};
use tablecraft_ace::{PropertyGroupKind, TableProperties};

const FORMAT: &str = "Format";
const DECLARATION_FLAGS: u8 = 1;
const NATIVE_TEXT: u8 = 10;
const TEXT_BOX: i16 = 109;

pub(super) fn apply(
    properties: Option<&TableProperties>,
    columns: &[crate::Column],
    rows: &mut [Vec<Cell>],
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    let Some(properties) = properties else {
        return Ok(());
    };
    for (column, name) in columns.iter().enumerate() {
        output.work(1)?;
        let Some((yes, no)) = labels(properties, &name.name, output)? else {
            continue;
        };
        for row in rows.iter_mut() {
            output.work(1)?;
            let Some(cell) = row.get_mut(column) else {
                return Err(tablecraft_ace::InspectError::Malformed.into());
            };
            format_cell(cell, yes, no, output)?;
        }
    }
    Ok(())
}

fn format_cell(
    cell: &mut Cell,
    yes: &str,
    no: &str,
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    if let CellKind::Scalar {
        is_null: false,
        boolean: Some(value),
        display_control: DisplayControl::Native(TEXT_BOX),
        ..
    } = cell.kind
    {
        cell.text = output.text(if value { yes } else { no })?;
    }
    Ok(())
}

fn labels(
    properties: &TableProperties,
    column: &str,
    output: &mut Presentation<'_>,
) -> Result<Option<(&'static str, &'static str)>, Error> {
    for group in properties.groups() {
        output.work(1 + group.name().len() as u64 + column.len() as u64)?;
        if group.kind() != PropertyGroupKind::Column || !group.name().eq_ignore_ascii_case(column) {
            continue;
        }
        for property in group.properties() {
            output.work(1 + property.name().len() as u64 + FORMAT.len() as u64)?;
            if !property.name().eq_ignore_ascii_case(FORMAT) {
                continue;
            }
            if property.flags() != DECLARATION_FLAGS || property.native_type() != NATIVE_TEXT {
                return Ok(None);
            }
            let format = property.text()?;
            // Built-in names are recognized case-insensitively. English labels
            // are explicit UI policy, not a claim about localized Access output.
            for (name, yes, no) in [
                ("Yes/No", "Yes", "No"),
                ("True/False", "True", "False"),
                ("On/Off", "On", "Off"),
            ] {
                output.work(1 + format.len() as u64 + name.len() as u64)?;
                if format.eq_ignore_ascii_case(name) {
                    return Ok(Some((yes, no)));
                }
            }
            return Ok(None);
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::Value;

    #[test]
    fn typed_boolean_labels_preserve_null_and_cached_facts_without_parsing_text() {
        let mut output = Presentation::new(&|| false);
        for value in [
            Value::Boolean(false),
            Value::Boolean(true),
            Value::Null,
            Value::Text("true".into()),
        ] {
            let mut cell = output.scalar(&value, true).unwrap();
            if let CellKind::Scalar {
                display_control, ..
            } = &mut cell.kind
            {
                *display_control = DisplayControl::Native(TEXT_BOX);
            }
            let original_kind = cell.kind;
            let original_text = cell.text.clone();
            format_cell(&mut cell, "Yes", "No", &mut output).unwrap();
            assert_eq!(cell.kind, original_kind);
            assert_eq!(
                cell.text,
                match value {
                    Value::Boolean(true) => "Yes".to_owned(),
                    Value::Boolean(false) => "No".to_owned(),
                    _ => original_text,
                }
            );
        }
    }
}
