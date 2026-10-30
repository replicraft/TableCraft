//! Original ordinal capture for guard-only SQL DELETE preparation.
use super::{Frame, copy};
use crate::{
    EditError, NativeReadSource, TableSnapshot,
    format::reader::Reader,
    header::check,
    table::mutation::{
        guards::{BeforeEventGuardError, buffer, error},
        rows,
    },
};

struct Ordinal(usize);

impl rows::RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

pub(super) fn append<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    table: &TableSnapshot,
    ordinals: &[usize],
    output: &mut Vec<Frame>,
) -> Result<(), BeforeEventGuardError> {
    let mut selected = buffer::vector(ordinals.len(), reader.budget, reader.cancelled)?;
    for &ordinal in ordinals {
        check(reader.budget, reader.cancelled)?;
        selected.push(Ordinal(ordinal));
    }
    rows::unique(&selected, reader.budget, reader.cancelled).map_err(error::intent)?;
    let cancelled = reader.cancelled;
    rows::visit_with_budget(reader, table.schema(), &selected, |budget, _, ordinal| {
        let original = table.rows().get(ordinal.0).ok_or(EditError::Conflict)?;
        let original = copy(original, table.columns().len(), budget, cancelled)?;
        output.push(Frame {
            ordinal: ordinal.0,
            original: Some(original),
            proposed: None,
        });
        Ok(())
    })
    .map_err(error::intent)
}
