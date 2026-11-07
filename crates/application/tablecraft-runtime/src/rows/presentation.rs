//! Presentation lineage for known-origin native table rows.
//! Arbitrary SQL results cannot opt into native row identity through this helper.
use super::{controls, formatting};
use crate::{Cell, Column, Error, controls::SCOPE, display::Presentation};
use crate::{DisplayControl, find::FindError};
use tablecraft_ace::{
    InspectError, NativeReadSource, TableProperties, TableSchema, inspect_table_schema,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_display::{NativeFormat, ScalarPresentation};
use tablecraft_schema::ScalarType;

const INPUT_MASK: &str = "InputMask";
const FORMAT: &str = "Format";
const DECLARATION_FLAGS: u8 = 1;
const TEXT_BOX_CONTROL: i16 = 109;

pub(crate) struct Metadata {
    schema: TableSchema,
    properties: Option<TableProperties>,
}

impl Metadata {
    pub fn inspect<R: NativeReadSource>(
        source: &mut R,
        table: &str,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let properties = crate::native_properties::inspect(source, table, work, cancelled)?;
        let schema = inspect_table_schema(source, SCOPE, table, work, cancelled)?;
        crate::native_properties::require_binding(properties.as_ref(), &schema)?;
        Ok(Self { schema, properties })
    }

    pub fn column(
        &self,
        position: usize,
        name: &str,
        output: &mut Presentation<'_>,
    ) -> Result<Column, Error> {
        output.work(1)?;
        let native = self
            .schema
            .columns()
            .get(position)
            .ok_or(InspectError::Malformed)?;
        output.work(native.name().len() as u64 + name.len() as u64)?;
        if native.name() != name {
            return Err(InspectError::Malformed.into());
        }

        crate::column::native(name, self.properties.as_ref(), output)
    }

    #[cfg(all(
        feature = "offline-edit",
        any(target_os = "macos", target_os = "linux", windows)
    ))]
    pub(crate) fn require_replace_control(
        &self,
        name: &str,
        output: &mut Presentation<'_>,
    ) -> Result<(), Error> {
        let control = controls::field(self.properties.as_ref(), name, output)?;
        if matches!(control, DisplayControl::Native(code) if code != TEXT_BOX_CONTROL)
            || control == DisplayControl::Unsupported
        {
            return Err(crate::editing::ReplaceError::UnsupportedControl.into());
        }
        Ok(())
    }

    pub(crate) fn require_find_format(
        &self,
        position: usize,
        name: &str,
        scalar: ScalarType,
        formatted: bool,
        output: &mut Presentation<'_>,
    ) -> Result<(), Error> {
        self.column(position, name, output)?;
        let control = controls::field(self.properties.as_ref(), name, output)?;
        if formatted && matches!(control, DisplayControl::Native(code) if code != TEXT_BOX_CONTROL)
            || control == DisplayControl::Unsupported
        {
            return Err(FindError::UnsupportedFormat.into());
        }
        let Some(properties) = &self.properties else {
            return Ok(());
        };
        for group in properties.groups() {
            output.work(1 + group.name().len() as u64 + name.len() as u64)?;
            if group.kind() != tablecraft_ace::PropertyGroupKind::Column
                || !group.name().eq_ignore_ascii_case(name)
            {
                continue;
            }
            for property in group.properties() {
                output.work(1 + property.name().len() as u64)?;
                let mask = property.name().eq_ignore_ascii_case(INPUT_MASK);
                let format = formatted && property.name().eq_ignore_ascii_case(FORMAT);
                if !mask && !format {
                    continue;
                }
                if property.flags() != DECLARATION_FLAGS {
                    return Err(FindError::UnsupportedFormat.into());
                }
                let text = match property.text() {
                    Ok(text) => text,
                    Err(InspectError::Unsupported) => {
                        return Err(FindError::UnsupportedFormat.into());
                    }
                    Err(error) => return Err(error.into()),
                };
                if text.is_empty() {
                    continue;
                }
                if mask {
                    return Err(FindError::UnsupportedFormat.into());
                }
                if !matches!(
                    scalar,
                    ScalarType::Currency | ScalarType::DateTime | ScalarType::DateTimeExtended
                ) || formatting::native_format(properties, name, output)?
                    == NativeFormat::Unsupported
                {
                    return Err(FindError::UnsupportedFormat.into());
                }
            }
        }
        Ok(())
    }

    /// Apply a known field's display format to a derived value, without row
    /// identity, checkbox controls or stored-calculation provenance.
    pub(crate) fn format_derived(
        &self,
        position: usize,
        name: &str,
        cell: &mut Cell,
        output: &mut Presentation<'_>,
    ) -> Result<(), Error> {
        self.column(position, name, output)?;
        let native_format = match &self.properties {
            Some(properties) => formatting::native_format(properties, name, output)?,
            None => NativeFormat::Unspecified,
        };
        if let Some(presentation) = &mut cell.presentation {
            match presentation {
                ScalarPresentation::Currency {
                    native_format: target,
                    ..
                }
                | ScalarPresentation::CurrencyAverage {
                    native_format: target,
                    ..
                }
                | ScalarPresentation::DateTime {
                    native_format: target,
                    ..
                } => *target = native_format,
            }
        }
        Ok(())
    }

    pub(crate) fn enrich_projection(
        &self,
        positions: &[usize],
        columns: &[Column],
        rows: &mut [Vec<Cell>],
        output: &mut Presentation<'_>,
    ) -> Result<(), Error> {
        if positions.len() != columns.len() {
            return Err(InspectError::Malformed.into());
        }
        let mut previous = None;
        for (&position, column) in positions.iter().zip(columns) {
            output.work(1)?;
            if previous.is_some_and(|old| old >= position)
                || self
                    .schema
                    .columns()
                    .get(position)
                    .is_none_or(|field| field.name() != column.name)
            {
                return Err(InspectError::Malformed.into());
            }
            previous = Some(position);
        }
        for row in rows.iter() {
            output.work(1)?;
            if row.len() != columns.len() {
                return Err(InspectError::Malformed.into());
            }
        }
        controls::enrich(self.properties.as_ref(), columns, rows, output)?;
        formatting::apply(self.properties.as_ref(), columns, rows, output)
    }

    #[cfg(all(
        feature = "offline-edit",
        any(target_os = "macos", target_os = "linux", windows)
    ))]
    pub fn enrich(
        &self,
        columns: &[Column],
        rows: &mut [Vec<Cell>],
        output: &mut Presentation<'_>,
    ) -> Result<(), Error> {
        output.work(1)?;
        if columns.len() != self.schema.columns().len() {
            return Err(InspectError::Malformed.into());
        }

        controls::enrich(self.properties.as_ref(), columns, rows, output)?;
        formatting::apply(self.properties.as_ref(), columns, rows, output)
    }
}
