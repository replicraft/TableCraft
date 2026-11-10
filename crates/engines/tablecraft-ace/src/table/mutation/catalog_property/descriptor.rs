//! Actual catalog object-to-LvProp descriptor observation, without write authority.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
use crate::format::row::reference_page;
use crate::{
    InspectError, NativeObjectOrigin, NativeReadSource, TableSchema,
    format::{
        data_type::OLE,
        layout::Layout,
        reader::{PhysicalRow, Reader},
    },
    table::{long_value::trace, record_layout},
};
use std::ops::Range;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
use tablecraft_core::limits::Resource;

/// The actual native record owns the observed bytes; absence is not inline-empty.
pub(crate) struct Descriptor {
    record: PhysicalRow,
    range: Option<Range<usize>>,
}

pub(crate) fn capture<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    catalog: &TableSchema,
    column: usize,
    origin: &NativeObjectOrigin,
) -> Result<Descriptor, InspectError> {
    let page = reader.page(origin.page())?;
    let record = reader.row_location(
        origin.page(),
        origin.slot(),
        catalog.definition_page(),
        &page,
    )?;
    finish(record, catalog, column, reader)
}

/// A retained row-purpose ledger supplies the complete checked catalog DATA
/// scope. Each followed native reference remains inside that same owner, with
/// paid dynamic cycle detection; the ledger separately proves unique terminals.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(crate) fn capture_retained<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    catalog: &TableSchema,
    column: usize,
    origin: &NativeObjectOrigin,
    owned: &[u32],
) -> Result<Descriptor, InspectError> {
    for pair in owned.windows(2) {
        reader.tick()?;
        if pair[0] >= pair[1] {
            return Err(InspectError::Malformed);
        }
    }
    let page = reader.page(origin.page())?;
    let record = reader.read_location_observed(
        origin.page(),
        origin.slot(),
        catalog.definition_page(),
        &page,
        &mut |reference, budget| {
            let levels = owned.len().checked_ilog2().unwrap_or(0) as u64 + 1;
            budget.charge(Resource::Steps, levels)?;
            if owned.binary_search(&reference_page(reference)).is_err() {
                return Err(InspectError::Malformed);
            }
            Ok(())
        },
    )?;
    finish(record, catalog, column, reader)
}

fn finish<R: NativeReadSource>(
    record: PhysicalRow,
    catalog: &TableSchema,
    column: usize,
    reader: &mut Reader<'_, R>,
) -> Result<Descriptor, InspectError> {
    let framed = record_layout::parse(&record.data, catalog, reader.budget)?;
    let column = catalog
        .columns()
        .get(column)
        .ok_or(InspectError::Malformed)?;
    let field = column.physical_field();
    if field.fixed
        || column.native_type() != OLE
        || framed.count != catalog.layout.max_columns
        || framed.offsets.len() != catalog.layout.variable_slots + 1
        || field.number >= framed.count
        || field.variable >= catalog.layout.variable_slots
    {
        return Err(InspectError::Unsupported);
    }
    let mask = record
        .data
        .get(framed.mask_start + field.number / u8::BITS as usize)
        .ok_or(InspectError::Malformed)?;
    if mask & (1 << (field.number % u8::BITS as usize)) == 0 {
        return Ok(Descriptor {
            record,
            range: None,
        });
    }
    let descriptor = record
        .data
        .get(framed.offsets[field.variable]..framed.offsets[field.variable + 1])
        .ok_or(InspectError::Malformed)?;
    reader.work(descriptor.len())?;
    let range = framed.offsets[field.variable]..framed.offsets[field.variable + 1];
    Ok(Descriptor {
        record,
        range: Some(range),
    })
}

impl Descriptor {
    fn bytes(&self) -> &[u8] {
        match &self.range {
            None => &[],
            // NO_CLONE: Return a borrowed descriptor slice without moving its range out of
            // the descriptor owner. Only range bounds are copied, not the
            // payload.
            Some(range) => &self.record.data[range.clone()],
        }
    }

    pub(in crate::table) fn kind(&self) -> Result<trace::Kind, InspectError> {
        trace::kind(self.bytes())
    }

    pub(crate) fn external(&self) -> Result<bool, InspectError> {
        Ok(matches!(
            trace::kind(self.bytes())?,
            trace::Kind::Single | trace::Kind::Chained
        ))
    }

    pub(super) fn matches_origin<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
        layout: &Layout,
        origin: &NativeObjectOrigin,
        name: &str,
    ) -> Result<bool, InspectError> {
        let actual = crate::catalog::record::decode(&self.record.data, layout, reader.budget)?;
        Ok(actual.id == origin.native_id()
            && actual.parent == origin.parent_id()
            && actual.kind == origin.type_code()
            && actual.flags == origin.flags()
            && actual.name == name)
    }

    /// Observe the actual physical descriptor frame, including a forwarded
    /// record's resolved location. This grants no mutation or role authority.
    pub(crate) fn absolute_range(&self) -> Result<Option<Range<usize>>, InspectError> {
        let Some(range) = &self.range else {
            return Ok(None);
        };
        let start = self
            .record
            .offset
            .checked_add(range.start)
            .ok_or(InspectError::Malformed)?;
        let end = self
            .record
            .offset
            .checked_add(range.end)
            .ok_or(InspectError::Malformed)?;
        Ok(Some(start..end))
    }

    pub(super) fn physical_offset(&self) -> usize {
        self.record.offset
    }

    pub(in crate::table) fn traced<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
    ) -> Result<trace::Traced, InspectError> {
        trace::read(reader, self.bytes())
    }

    /// Strict original producer binding; no LVAL page or body is followed here.
    pub(crate) fn canonical_single(
        &self,
        reference: u32,
        bytes: usize,
    ) -> Result<bool, InspectError> {
        trace::canonical_single(self.bytes(), reference, bytes)
    }
}

#[cfg(test)]
mod tests;
