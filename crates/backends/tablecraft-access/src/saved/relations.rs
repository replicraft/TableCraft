use super::{
    SavedSelectError,
    context::{Context, check_name},
    fragments::{Fragments, MAX_SOURCES},
    join_conditions,
};
use tablecraft_ace::SavedQueryRow;
use tablecraft_sql::select::{JoinKind, Table, TableSource};

const INNER_JOIN: i16 = 1;
const LEFT_JOIN: i16 = 2;
const RIGHT_JOIN: i16 = 3;

struct Source<'a> {
    name: &'a str,
    alias: Option<&'a str>,
}
impl<'a> Source<'a> {
    fn read(row: &'a SavedQueryRow) -> Result<Self, SavedSelectError> {
        let name = row.name1().ok_or(SavedSelectError::InvalidMetadata)?;
        check_name(name)?;
        let alias = row.name2();
        if let Some(alias) = alias {
            check_name(alias)?;
        }
        Ok(Self { name, alias })
    }
    fn label(&self) -> &str {
        self.alias.unwrap_or(self.name)
    }
}
pub(super) fn read(
    fragments: &Fragments<'_>,
    context: &mut Context<'_>,
) -> Result<Vec<TableSource>, SavedSelectError> {
    context.check()?;
    let count = fragments.table_count;
    let [first, second] = fragments.tables;
    if count < MAX_SOURCES {
        if fragments.join_count != 0 {
            return Err(SavedSelectError::InvalidMetadata);
        }
        let mut from = context.reserve(count)?;
        if let Some(row) = first {
            let source = Source::read(row)?;
            from.push(TableSource::Table(Table {
                name: context.name(source.name)?,
                alias: source.alias.map(|s| context.name(s)).transpose()?,
            }));
        }
        return Ok(from);
    }
    let join = fragments
        .joins
        .first()
        .copied()
        .flatten()
        .ok_or(SavedSelectError::Unsupported)?;
    let first = Source::read(first.ok_or(SavedSelectError::InvalidMetadata)?)?;
    let second = Source::read(second.ok_or(SavedSelectError::InvalidMetadata)?)?;
    let left = join.name1().ok_or(SavedSelectError::InvalidMetadata)?;
    let right = join.name2().ok_or(SavedSelectError::InvalidMetadata)?;
    // The reference uses case-insensitive source aliases (or names without an
    // alias). Restrict the qualified comparison profile to ASCII labels.
    if [first.label(), second.label(), left, right]
        .iter()
        .any(|s| !s.is_ascii())
    {
        return Err(SavedSelectError::Unsupported);
    }
    if first.label().eq_ignore_ascii_case(second.label()) || left.eq_ignore_ascii_case(right) {
        return Err(SavedSelectError::InvalidMetadata);
    }
    let (left_source, right_source) = if first.label().eq_ignore_ascii_case(left)
        && second.label().eq_ignore_ascii_case(right)
    {
        (&first, &second)
    } else if second.label().eq_ignore_ascii_case(left) && first.label().eq_ignore_ascii_case(right)
    {
        (&second, &first)
    } else {
        return Err(SavedSelectError::InvalidMetadata);
    };
    let (kind, operator) = match join.flag() {
        Some(INNER_JOIN) => (JoinKind::Inner, " INNER JOIN ["),
        Some(LEFT_JOIN) => (JoinKind::Left, " LEFT JOIN ["),
        Some(RIGHT_JOIN) => (JoinKind::Right, " RIGHT JOIN ["),
        Some(_) => return Err(SavedSelectError::Unsupported),
        None => return Err(SavedSelectError::InvalidMetadata),
    };
    let condition = join_conditions::read(fragments, context)?;
    // Validate independently before insertion: a stored clause must not close
    // our ON parentheses and introduce an outer WHERE or another source.
    let expected = context.expression(&condition)?;
    let mut query = context.select(&[
        "SELECT 0 FROM [",
        left_source.name,
        "]",
        if left_source.alias.is_some() {
            " AS ["
        } else {
            ""
        },
        left_source.alias.unwrap_or(""),
        if left_source.alias.is_some() { "]" } else { "" },
        operator,
        right_source.name,
        "]",
        if right_source.alias.is_some() {
            " AS ["
        } else {
            ""
        },
        right_source.alias.unwrap_or(""),
        if right_source.alias.is_some() {
            "]"
        } else {
            ""
        },
        " ON (",
        &condition,
        "\n)",
    ])?;
    if query.selection.is_some() {
        return Err(SavedSelectError::InvalidMetadata);
    }
    match query.from.as_slice() {
        [
            TableSource::Join {
                kind: actual,
                condition,
                ..
            },
        ] if *actual == kind && *condition == expected => {}
        _ => return Err(SavedSelectError::InvalidMetadata),
    }
    context.check()?;
    Ok(std::mem::take(&mut query.from))
}
