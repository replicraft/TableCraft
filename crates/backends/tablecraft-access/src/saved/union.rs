//! Labelled native UNION fragments, without provider activation or SQL execution.
use super::{
    SavedRead, SavedReadError as E,
    context::Context,
    fragments::{expression, no_payload, ordered, unique, zero_flag},
    parameters,
};
use tablecraft_ace::{QueryAttribute as A, SavedQueryDefinition, SavedQueryRow};
use tablecraft_core::limits::Resource;
use tablecraft_sql::{query::Statement, select::SortKey};

pub(super) const FAMILY: u32 = 0x80;
const TYPE: i16 = 9;
const PART_COUNT: usize = 2;
const DISTINCT: i16 = 0x02;
const OWNER_ACCESS: i16 = 0x04;
const KNOWN_FLAGS: i16 = DISTINCT | OWNER_ACCESS;
const FIRST_PART: &str = "X7YZ_____1";
const SECOND_PART: &str = "X7YZ_____2";

struct Fragments<'a> {
    parts: [Option<&'a SavedQueryRow>; PART_COUNT],
    flags: Option<&'a SavedQueryRow>,
    parameters: usize,
    orders: usize,
}
fn validate<'a>(
    definition: &'a SavedQueryDefinition,
    context: &mut Context<'_>,
) -> Result<Fragments<'a>, E> {
    let mut result = Fragments {
        parts: [None, None],
        flags: None,
        parameters: 0,
        orders: 0,
    };
    let (mut start, mut end, mut kind) = (None, None, None);
    let (mut parameter_order, mut sort_order) = (None, None);
    for row in definition.rows() {
        context.check()?;
        context.budget.charge(Resource::Rows, 1)?;
        if row.extra().is_some() && row.attribute() != Some(A::Parameter) {
            return Err(E::Unsupported);
        }
        match row.attribute() {
            Some(A::Start | A::End) => {
                no_payload(row)?;
                zero_flag(row)?;
                unique(
                    if row.attribute() == Some(A::Start) {
                        &mut start
                    } else {
                        &mut end
                    },
                    row,
                )?;
            }
            Some(A::Type) => {
                no_payload(row)?;
                if row.flag() != Some(TYPE) {
                    return Err(E::InvalidMetadata);
                }
                unique(&mut kind, row)?;
            }
            Some(A::Flags) => {
                no_payload(row)?;
                if row.flag().unwrap_or(0) & !KNOWN_FLAGS != 0 {
                    return Err(E::Unsupported);
                }
                unique(&mut result.flags, row)?;
            }
            Some(A::Table) => {
                zero_flag(row)?;
                expression(row)?;
                if row.name1().is_some() {
                    return Err(E::Unsupported);
                }
                let slot = match row.name2() {
                    Some(FIRST_PART) => 0,
                    Some(SECOND_PART) => 1,
                    _ => return Err(E::Unsupported),
                };
                unique(&mut result.parts[slot], row)?;
            }
            Some(A::Parameter) => {
                if row.expression().is_some() || row.name2().is_some() {
                    return Err(E::Unsupported);
                }
                if result.parameters == parameters::MAX_DECLARATIONS {
                    return Err(E::SyntaxLimit);
                }
                ordered(row, &mut parameter_order)?;
                result.parameters += 1;
            }
            Some(A::OrderBy) => {
                zero_flag(row)?;
                expression(row)?;
                if row.name2().is_some()
                    || row.name1().is_some_and(|s| {
                        !s.eq_ignore_ascii_case("A") && !s.eq_ignore_ascii_case("D")
                    })
                {
                    return Err(E::Unsupported);
                }
                ordered(row, &mut sort_order)?;
                result.orders += 1;
            }
            _ => return Err(E::Unsupported),
        }
    }
    if kind.is_none() || result.parts.iter().any(Option::is_none) {
        return Err(E::InvalidMetadata);
    }
    Ok(result)
}

pub(super) fn reconstruct(
    definition: SavedQueryDefinition,
    context: &mut Context<'_>,
) -> Result<SavedRead, E> {
    context.check()?;
    let fragments = validate(&definition, context)?;
    let first = expression(fragments.parts[0].ok_or(E::InvalidMetadata)?)?;
    let second = expression(fragments.parts[1].ok_or(E::InvalidMetadata)?)?;
    // A UNION wrapper admits named-query operands without treating raw names as
    // standalone statements. Each part must remain a read body, with no injected
    // declarations, final ORDER BY or owner option. Newlines end line comments.
    for parts in [
        &[first, "\nUNION ALL SELECT 0"][..],
        &["SELECT 0 UNION ALL\n", second, "\nUNION ALL SELECT 0"][..],
    ] {
        let query = context.query(parts)?;
        require_body(&query)?;
    }
    let flags = fragments.flags.and_then(SavedQueryRow::flag).unwrap_or(0);
    let operator = if flags & DISTINCT != 0 {
        "\nUNION\n"
    } else {
        "\nUNION ALL\n"
    };
    // Parsing the complete composition also enforces combined byte/token/depth
    // limits, statement boundaries and left-to-right mixed UNION/ALL association.
    let mut query = context.query(&[first, operator, second, "\n"])?;
    require_body(&query)?;
    let Statement::Read(read) = &mut query.statement else {
        return Err(E::Unsupported);
    };
    let mut order_by = context.reserve(fragments.orders)?;
    for row in definition.rows() {
        context.check()?;
        if row.attribute() == Some(A::OrderBy) {
            order_by.push(SortKey {
                expression: context.expression(expression(row)?)?,
                descending: row.name1().is_some_and(|s| s.eq_ignore_ascii_case("D")),
            });
        }
    }
    read.order_by = order_by;
    read.owner_access = flags & OWNER_ACCESS != 0;
    query.parameters = parameters::read(&definition, fragments.parameters, context)?;
    context.check()?;
    Ok(SavedRead { definition, query })
}
fn require_body(query: &tablecraft_sql::query::Query) -> Result<(), E> {
    let Statement::Read(read) = &query.statement else {
        return Err(E::Unsupported);
    };
    if !query.parameters.is_empty() || !read.order_by.is_empty() || read.owner_access {
        return Err(E::Unsupported);
    }
    Ok(())
}
