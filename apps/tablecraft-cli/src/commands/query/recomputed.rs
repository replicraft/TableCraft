//! Group explicit CLI field selections; the adapter validates and owns each view.
use super::Error;
use tablecraft_access::CalculationView;

pub(super) const SELECTION_FIELDS: usize = 3;
const MAX_SELECTIONS: usize = 256;

pub(super) struct Selection<'a> {
    alias: &'a str,
    table: &'a str,
    columns: Vec<&'a str>,
}

pub(super) fn group(raw: &[String]) -> Result<Vec<Selection<'_>>, Error> {
    let (entries, remainder) = raw.as_chunks::<SELECTION_FIELDS>();
    if !remainder.is_empty() || entries.len() > MAX_SELECTIONS {
        return Err(Error::Parameter);
    }

    let mut selected: Vec<Selection<'_>> = Vec::new();
    for [alias, table, column] in entries {
        match selected
            .iter_mut()
            .find(|selection| selection.alias.eq_ignore_ascii_case(alias))
        {
            Some(selection) => {
                if !selection.table.eq_ignore_ascii_case(table) {
                    return Err(Error::Parameter);
                }
                selection.columns.push(column);
            }
            None => selected.push(Selection {
                alias,
                table,
                columns: vec![column],
            }),
        }
    }
    Ok(selected)
}

pub(super) fn requests<'a>(selections: &'a [Selection<'a>]) -> Vec<CalculationView<'a>> {
    selections
        .iter()
        .map(|selection| CalculationView::new(selection.alias, selection.table, &selection.columns))
        .collect()
}
