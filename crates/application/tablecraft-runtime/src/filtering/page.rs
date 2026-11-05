//! Read one original band once and present only its checked visible membership.
use super::{FilterHandle, FilterTarget, FilteredData, SortSpec};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
use tablecraft_access::{CollectionPageRequest, NativeBrowser, TablePageRequest};
use tablecraft_ace::{InspectError, TableSnapshotLimits, inspect_table_schema};

const VISIBLE_PAGE_ROWS: u32 = 100;

impl Document {
    pub(crate) fn filter_page(
        &mut self,
        handle: &FilterHandle,
        offset: u32,
        enabled: bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        self.check_filter(handle)?;
        let enabled = enabled && handle.filter_count() != 0;
        let handle = self.filter_for_mode(handle, enabled, cancelled)?;
        let count = if enabled {
            handle.matched_rows()
        } else {
            handle.source_rows()
        };
        if offset > count {
            return Err(InspectError::InvalidPageRange.into());
        }

        let returned = (count - offset).min(VISIBLE_PAGE_ROWS);
        let mut output = Presentation::new(cancelled);
        output.allocation(returned as usize, size_of::<u32>())?;
        let mut ordinals = Vec::new();
        ordinals.try_reserve_exact(returned as usize)?;
        for visible in offset..offset + returned {
            output.work(1)?;
            ordinals.push(
                handle
                    .display_ordinal(enabled, visible)
                    .ok_or(InspectError::Malformed)?,
            );
        }
        // The map scan and band read are separately named bounded phases. This
        // native allowance is never reset per selected row, field or collection.
        let mut page_work = budget();
        let properties = crate::native_properties::inspect(
            &mut self.source,
            &handle.target().table,
            &mut page_work,
            cancelled,
        )?;
        output.work(ordinals.len() as u64 * 2)?;
        let (columns, rows) = if let (Some(first), Some(last)) = (
            ordinals.iter().copied().min(),
            ordinals.iter().copied().max(),
        ) {
            let width = last
                .checked_sub(first)
                .and_then(|width| width.checked_add(1))
                .ok_or(InspectError::InvalidPageRange)?;
            output.allocation(ordinals.len(), size_of::<usize>())?;
            let mut selected = Vec::new();
            selected.try_reserve_exact(ordinals.len())?;
            for &ordinal in &ordinals {
                output.work(1)?;
                selected.push(
                    usize::try_from(ordinal - first).map_err(|_| InspectError::InvalidPageRange)?,
                );
            }
            let mut browser = NativeBrowser::new(&mut self.source, SCOPE, page_work, cancelled);
            let request = CollectionPageRequest::new(
                TablePageRequest::new(first, width)?,
                TableSnapshotLimits::default(),
            );
            let page = browser.browse_page(&handle.target().table, request)?;
            let page = crate::rows::page::present(
                &page,
                properties.as_ref(),
                Some(&selected),
                &mut output,
            )?;
            (page.columns, page.rows)
        } else {
            // An empty view publishes declarations, not an unrelated source row.
            let schema = inspect_table_schema(
                &mut self.source,
                SCOPE,
                &handle.target().table,
                &mut page_work,
                cancelled,
            )?;
            crate::native_properties::require_binding(properties.as_ref(), &schema)?;
            let mut columns = output.columns(schema.columns().len())?;
            for column in schema.columns() {
                columns.push(crate::column::native(
                    column.name(),
                    properties.as_ref(),
                    &mut output,
                )?);
            }
            (columns, output.rows(0)?)
        };
        if rows.len() != ordinals.len() {
            return Err(InspectError::Malformed.into());
        }
        let target = FilterTarget {
            document_revision: handle.target().document_revision,
            table: output.text(&handle.target().table)?,
            filter_id: handle.target().filter_id,
        };
        check(cancelled)?;
        Ok(Response::FilteredData(FilteredData {
            handle: handle.clone(),
            target,
            enabled,
            offset,
            columns,
            rows,
            source_ordinals: ordinals,
            matched_rows: handle.matched_rows(),
            source_rows: handle.source_rows(),
            has_more: offset + returned < count,
            sort: handle
                .sort()
                .map(|sort| -> Result<SortSpec, Error> {
                    Ok(SortSpec {
                        column: output.text(&sort.column)?,
                        descending: sort.descending,
                    })
                })
                .transpose()?,
            filter_count: handle.filter_count(),
        }))
    }
}
