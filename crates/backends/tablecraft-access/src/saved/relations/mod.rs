//! Bounded native join forests, independent of binding and user-row reads.
mod conditions;
mod forest;
mod render;
mod source;
use super::{SavedSelectError as E, context::Context, fragments::Fragments};
use source::Source;
use tablecraft_ace::SavedQueryRow;
use tablecraft_sql::select::{JoinKind, TableSource};

const INNER: i16 = 1;
const LEFT: i16 = 2;
const RIGHT: i16 = 3;

fn kind(row: &SavedQueryRow) -> Result<JoinKind, E> {
    match row.flag() {
        Some(INNER) => Ok(JoinKind::Inner),
        Some(LEFT) => Ok(JoinKind::Left),
        Some(RIGHT) => Ok(JoinKind::Right),
        None => Err(E::InvalidMetadata),
        _ => Err(E::Unsupported),
    }
}
pub(super) fn read(
    fragments: &Fragments<'_>,
    context: &mut Context<'_>,
) -> Result<Vec<TableSource>, E> {
    context.check()?;
    if fragments.table_count == 0 {
        if fragments.join_count != 0 {
            return Err(E::InvalidMetadata);
        }
        return context.reserve(0);
    }
    let forest = forest::Forest::build(fragments, context)?;
    render::read(&forest, context)
}
