//! Field identity and optional inert native header presentation.
use crate::{Error, display::Presentation};
use tablecraft_ace::{PropertyGroupKind, TableProperties};

const CAPTION: &str = "Caption";
const DECLARATION_FLAGS: u8 = 1;
const NATIVE_TEXT: u8 = 10;
const NATIVE_MEMO: u8 = 12;

/// An exact result field name, with independent optional display metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// Schema field name or query output alias; used for all field identities.
    pub name: String,
    /// Explicit qualified native Caption, including an explicitly empty value.
    /// This must never replace `name` in a target or metadata lookup.
    pub caption: Option<String>,
}

/// Publish a query alias or a synthetic child heading without native metadata.
pub(super) fn plain(name: &str, output: &mut Presentation<'_>) -> Result<Column, Error> {
    Ok(Column {
        name: output.text(name)?,
        caption: None,
    })
}

pub(super) fn native(
    name: &str,
    properties: Option<&TableProperties>,
    output: &mut Presentation<'_>,
) -> Result<Column, Error> {
    let name = output.text(name)?;
    let caption = caption(&name, properties, output)?;
    Ok(Column { name, caption })
}

fn caption(
    name: &str,
    properties: Option<&TableProperties>,
    output: &mut Presentation<'_>,
) -> Result<Option<String>, Error> {
    let Some(properties) = properties else {
        return Ok(None);
    };
    for group in properties.groups() {
        output.work(1 + group.name().len() as u64 + name.len() as u64)?;
        if group.kind() != PropertyGroupKind::Column || !group.name().eq_ignore_ascii_case(name) {
            continue;
        }
        for property in group.properties() {
            output.work(1 + property.name().len() as u64 + CAPTION.len() as u64)?;
            if !property.name().eq_ignore_ascii_case(CAPTION) {
                continue;
            }
            if property.flags() != DECLARATION_FLAGS
                || !matches!(property.native_type(), NATIVE_TEXT | NATIVE_MEMO)
            {
                return Ok(None);
            }
            return Ok(Some(output.text(property.text()?)?));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
