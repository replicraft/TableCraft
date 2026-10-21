use super::{SavedSelectError, context::Context, crosstab, parameters::MAX_DECLARATIONS};
use tablecraft_ace::{QueryAttribute as A, SavedQueryDefinition, SavedQueryRow};
use tablecraft_core::limits::Resource;

pub(super) const QUERY_FAMILY_MASK: u32 = 0xf0;
pub(super) const SELECT_FAMILY: u32 = 0;
const SELECT_TYPE: i16 = 1;
const ORDER_BYTES: usize = size_of::<u32>();
pub(super) const MAX_SOURCES: usize = u16::BITS as usize;
// Qualified consumer ceiling; not a universal native engine condition limit.
pub(super) const MAX_JOIN_PARTS: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Profile {
    Select,
    Crosstab,
}

pub(super) struct Fragments<'a> {
    profile: Profile,
    pub transform: Option<&'a SavedQueryRow>,
    pub pivot: Option<&'a SavedQueryRow>,
    pub pivot_group: Option<&'a SavedQueryRow>,
    pub flags: Option<&'a SavedQueryRow>,
    pub tables: [Option<&'a SavedQueryRow>; MAX_SOURCES],
    pub table_count: usize,
    pub joins: [Option<&'a SavedQueryRow>; MAX_JOIN_PARTS],
    pub join_count: usize,
    pub selection: Option<&'a SavedQueryRow>,
    pub having: Option<&'a SavedQueryRow>,
    pub columns: usize,
    pub groups: usize,
    pub orders: usize,
    pub parameters: usize,
}
impl Fragments<'_> {
    pub fn is_row_column(&self, row: &SavedQueryRow) -> bool {
        self.profile == Profile::Select || row.flag() == Some(crosstab::ROW_FLAG)
    }
    pub fn is_row_group(&self, row: &SavedQueryRow) -> bool {
        self.is_row_column(row)
    }
}

pub(super) fn validate<'a>(
    definition: &'a SavedQueryDefinition,
    context: &mut Context<'_>,
) -> Result<Fragments<'a>, SavedSelectError> {
    validate_profile(definition, context, Profile::Select)
}
pub(super) fn validate_profile<'a>(
    definition: &'a SavedQueryDefinition,
    context: &mut Context<'_>,
    profile: Profile,
) -> Result<Fragments<'a>, SavedSelectError> {
    let (family, kind_code) = match profile {
        Profile::Select => (SELECT_FAMILY, SELECT_TYPE),
        Profile::Crosstab => (crosstab::FAMILY, crosstab::TYPE),
    };
    if definition.origin().flags() & QUERY_FAMILY_MASK != family {
        return Err(SavedSelectError::Unsupported);
    }
    let mut result = Fragments {
        profile,
        transform: None,
        pivot: None,
        pivot_group: None,
        flags: None,
        tables: [None; MAX_SOURCES],
        table_count: 0,
        joins: [None; MAX_JOIN_PARTS],
        join_count: 0,
        selection: None,
        having: None,
        columns: 0,
        groups: 0,
        orders: 0,
        parameters: 0,
    };
    let (mut start, mut end, mut kind) = (None, None, None);
    let mut column_order = None;
    let mut group_order = None;
    let mut sort_order = None;
    let mut parameter_order = None;
    let mut join_order = None;
    for row in definition.rows() {
        context.check()?;
        context.budget.charge(Resource::Rows, 1)?;
        if row.extra().is_some() && row.attribute() != Some(A::Parameter) {
            return Err(SavedSelectError::Unsupported);
        }
        match row.attribute() {
            Some(A::Start | A::End) => {
                no_payload(row)?;
                zero_flag(row)?;
                if row.attribute() == Some(A::Start) {
                    unique(&mut start, row)?;
                } else {
                    unique(&mut end, row)?;
                }
            }
            Some(A::Type) => {
                no_payload(row)?;
                if row.flag() != Some(kind_code) {
                    return Err(SavedSelectError::InvalidMetadata);
                }
                unique(&mut kind, row)?;
            }
            Some(A::Flags) => {
                if row.expression().is_some() || row.name2().is_some() {
                    return Err(SavedSelectError::Unsupported);
                }
                unique(&mut result.flags, row)?;
            }
            Some(A::Parameter) => {
                if row.expression().is_some() || row.name2().is_some() {
                    return Err(SavedSelectError::Unsupported);
                }
                if result.parameters == MAX_DECLARATIONS {
                    return Err(SavedSelectError::SyntaxLimit);
                }
                ordered(row, &mut parameter_order)?;
                result.parameters += 1;
            }
            Some(A::Table) => {
                // A named table plus expression denotes a remote qualifier;
                // an expression without a name is a stored derived query.
                if row.name1().is_some() && row.expression().is_some() {
                    return Err(SavedSelectError::Unsupported);
                }
                zero_flag(row)?;
                if result.table_count == MAX_SOURCES {
                    return Err(SavedSelectError::Unsupported);
                }
                result.tables[result.table_count] = Some(row);
                result.table_count += 1;
            }
            Some(A::Join) => {
                expression(row)?;
                if result.join_count == MAX_JOIN_PARTS {
                    return Err(SavedSelectError::SyntaxLimit);
                }
                ordered(row, &mut join_order)?;
                result.joins[result.join_count] = Some(row);
                result.join_count += 1;
            }
            Some(A::Column) => {
                if row.name2().is_some() {
                    return Err(SavedSelectError::Unsupported);
                }
                if profile == Profile::Select {
                    zero_flag(row)?;
                }
                expression(row)?;
                ordered(row, &mut column_order)?;
                if profile == Profile::Select {
                    result.columns += 1;
                } else {
                    match crosstab::column_role(row)? {
                        crosstab::Role::Row => result.columns += 1,
                        crosstab::Role::Transform => unique(&mut result.transform, row)?,
                        crosstab::Role::Pivot => unique(&mut result.pivot, row)?,
                    }
                }
            }
            Some(A::Where | A::Having | A::GroupBy | A::OrderBy) => {
                if row.name2().is_some()
                    || (row.attribute() != Some(A::OrderBy) && row.name1().is_some())
                {
                    return Err(SavedSelectError::Unsupported);
                }
                let crosstab_group =
                    profile == Profile::Crosstab && row.attribute() == Some(A::GroupBy);
                if crosstab_group {
                    crosstab::group_role(row)?;
                } else {
                    zero_flag(row)?;
                }
                expression(row)?;
                match row.attribute() {
                    Some(A::Where) => unique(&mut result.selection, row)?,
                    Some(A::Having) => unique(&mut result.having, row)?,
                    Some(A::GroupBy) => {
                        ordered(row, &mut group_order)?;
                        if crosstab_group && crosstab::group_role(row)? == crosstab::Role::Pivot {
                            unique(&mut result.pivot_group, row)?;
                        } else {
                            result.groups += 1;
                        }
                    }
                    Some(A::OrderBy) => {
                        if row.name1().is_some_and(|s| {
                            !s.eq_ignore_ascii_case("A") && !s.eq_ignore_ascii_case("D")
                        }) {
                            return Err(SavedSelectError::Unsupported);
                        }
                        ordered(row, &mut sort_order)?;
                        result.orders += 1;
                    }
                    _ => return Err(SavedSelectError::InvalidMetadata),
                }
            }
            _ => return Err(SavedSelectError::Unsupported),
        }
    }
    if definition.rows().is_empty()
        || (profile == Profile::Crosstab
            && (kind.is_none() || result.transform.is_none() || result.pivot.is_none()))
    {
        return Err(SavedSelectError::InvalidMetadata);
    }
    Ok(result)
}
pub(super) fn unique<'a>(
    slot: &mut Option<&'a SavedQueryRow>,
    row: &'a SavedQueryRow,
) -> Result<(), SavedSelectError> {
    if slot.replace(row).is_some() {
        return Err(SavedSelectError::InvalidMetadata);
    }
    Ok(())
}
pub(super) fn zero_flag(row: &SavedQueryRow) -> Result<(), SavedSelectError> {
    if row.flag().unwrap_or(0) != 0 {
        return Err(SavedSelectError::Unsupported);
    }
    Ok(())
}
pub(super) fn no_payload(row: &SavedQueryRow) -> Result<(), SavedSelectError> {
    if row.name1().is_some() || row.name2().is_some() || row.expression().is_some() {
        return Err(SavedSelectError::Unsupported);
    }
    Ok(())
}
pub(super) fn expression(row: &SavedQueryRow) -> Result<&str, SavedSelectError> {
    row.expression().ok_or(SavedSelectError::InvalidMetadata)
}
pub(super) fn ordered<'a>(
    row: &'a SavedQueryRow,
    previous: &mut Option<&'a [u8]>,
) -> Result<(), SavedSelectError> {
    let order = row
        .order()
        .filter(|s| s.len() == ORDER_BYTES)
        .ok_or(SavedSelectError::Unsupported)?;
    // This qualified profile requires storage order and native keys to agree.
    // Do not silently reorder duplicate/reversed/unknown order encodings.
    if previous.is_some_and(|before| before >= order) {
        return Err(SavedSelectError::InvalidMetadata);
    }
    *previous = Some(order);
    Ok(())
}
