//! Bounded native join forests, independent of binding and user-row reads.
mod conditions;
mod forest;
mod render;
use super::{
    SavedSelectError as E,
    context::{Context, check_name},
    fragments::Fragments,
};
use tablecraft_ace::SavedQueryRow;
use tablecraft_core::limits::Resource;
use tablecraft_sql::select::{JoinKind, TableSource};

const INNER: i16 = 1;
const LEFT: i16 = 2;
const RIGHT: i16 = 3;

pub(super) struct Source<'a> {
    name: &'a str,
    alias: Option<&'a str>,
}
impl<'a> Source<'a> {
    fn read(row: &'a SavedQueryRow, context: &mut Context<'_>) -> Result<Self, E> {
        let name = row.name1().ok_or(E::InvalidMetadata)?;
        let alias = row.name2();
        context.check()?;
        context.budget.charge(
            Resource::Steps,
            (name.len() + alias.map_or(0, str::len)) as u64,
        )?;
        check_name(name)?;
        if let Some(alias) = alias {
            check_name(alias)?;
        }
        Ok(Self { name, alias })
    }
    fn label(&self) -> &str {
        self.alias.unwrap_or(self.name)
    }
}
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
