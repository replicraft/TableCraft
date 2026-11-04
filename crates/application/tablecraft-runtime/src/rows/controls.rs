//! Inert native field-control metadata under the page operation's original budget.
use crate::{Cell, CellKind, DisplayControl, Error, controls::SCOPE, display::Presentation};
use tablecraft_ace::{
    InspectError, NativeReadSource, PropertyGroupKind, TableProperties, TableSchema,
    inspect_table_properties, inspect_table_schema,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG: &str = "MSysObjects";
const PROPERTY_CARRIER: &str = "LvProp";
const DISPLAY_CONTROL: &str = "DisplayControl";
const DECLARATION_FLAGS: u8 = 1;

pub(super) fn inspect<R: NativeReadSource>(
    source: &mut R,
    table: &str,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<TableProperties>, Error> {
    let catalog = inspect_table_schema(source, SCOPE, CATALOG, work, cancelled)?;
    for column in catalog.columns() {
        crate::controls::check(cancelled)?;
        work.charge(
            Resource::Steps,
            1 + column.name().len() as u64 + PROPERTY_CARRIER.len() as u64,
        )?;
        if column.name().eq_ignore_ascii_case(PROPERTY_CARRIER) {
            return Ok(Some(inspect_table_properties(
                source, SCOPE, table, work, cancelled,
            )?));
        }
    }
    Ok(None)
}

pub(super) fn require_binding(
    properties: Option<&TableProperties>,
    schema: &TableSchema,
) -> Result<(), Error> {
    if properties.is_some_and(|properties| properties.object_id() != schema.object_id()) {
        return Err(InspectError::Malformed.into());
    }
    Ok(())
}

pub(super) fn enrich(
    properties: Option<&TableProperties>,
    columns: &[String],
    rows: &mut [Vec<Cell>],
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    for (index, name) in columns.iter().enumerate() {
        output.work(1)?;
        let control = field(properties, name, output)?;
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
