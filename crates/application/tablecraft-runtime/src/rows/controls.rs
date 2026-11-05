//! Inert native field-control metadata under the page operation's original budget.
use crate::{Cell, CellKind, DisplayControl, Error, display::Presentation};
use tablecraft_ace::{InspectError, PropertyGroupKind, TableProperties};

const DISPLAY_CONTROL: &str = "DisplayControl";
const DECLARATION_FLAGS: u8 = 1;

pub(super) fn enrich(
    properties: Option<&TableProperties>,
    columns: &[crate::Column],
    rows: &mut [Vec<Cell>],
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    for (index, name) in columns.iter().enumerate() {
        output.work(1)?;
        let control = field(properties, &name.name, output)?;
        for row in rows.iter_mut() {
            output.work(1)?;
            let cell = row.get_mut(index).ok_or(InspectError::Malformed)?;
            if let CellKind::Scalar {
                display_control, ..
            } = &mut cell.kind
            {
                *display_control = control;
            }
        }
    }
    Ok(())
}

fn field(
    properties: Option<&TableProperties>,
    name: &str,
    output: &mut Presentation<'_>,
) -> Result<DisplayControl, Error> {
    let Some(properties) = properties else {
        return Ok(DisplayControl::Uninspected);
    };
    for group in properties.groups() {
        output.work(1 + group.name().len() as u64 + name.len() as u64)?;
        if group.kind() != PropertyGroupKind::Column || !group.name().eq_ignore_ascii_case(name) {
            continue;
        }
        for property in group.properties() {
            output.work(1 + property.name().len() as u64 + DISPLAY_CONTROL.len() as u64)?;
            if property.name().eq_ignore_ascii_case(DISPLAY_CONTROL) {
                return Ok(if property.flags() == DECLARATION_FLAGS {
                    match property.int16() {
                        Ok(code) => DisplayControl::Native(code),
                        Err(InspectError::Unsupported | InspectError::Malformed) => {
                            DisplayControl::Unsupported
                        }
                        Err(error) => return Err(error.into()),
                    }
                } else {
                    DisplayControl::Unsupported
                });
            }
        }
    }
    Ok(DisplayControl::Absent)
}
