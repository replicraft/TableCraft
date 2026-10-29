//! Checked nullable UNIQUE parent keys and immutable original associations.
use super::super::{
    EditError, reserve,
    state::{State, Table},
};
use crate::{
    InspectError, NativeRelationship, WriteBlocker, format::layout::INDEX_KEY_COLUMNS,
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::{ReferentialAction, ScalarType};

pub(super) struct Link {
    pub(super) parent: usize,
    pub(super) child: usize,
    pub(super) declaration: usize,
    pub(super) parents: [usize; INDEX_KEY_COLUMNS],
    pub(super) children: [usize; INDEX_KEY_COLUMNS],
    pub(super) width: usize,
    pub(super) action: ReferentialAction,
    pub(super) matches: Vec<(usize, usize)>,
}

fn column(table: &Table, name: &str, budget: &mut ResourceBudget) -> Result<usize, EditError> {
    budget
        .charge(
            Resource::Steps,
            table.before.schema().columns().len() as u64,
        )
        .map_err(InspectError::from)?;
    table
        .before
        .schema()
        .columns()
        .iter()
        .position(|column| {
            column.name().eq_ignore_ascii_case(name)
                && matches!(
                    column.scalar_type(),
                    Some(
                        ScalarType::Byte
                            | ScalarType::Int16
                            | ScalarType::Int32
                            | ScalarType::Int64
                            | ScalarType::Currency
                            | ScalarType::Guid
                            | ScalarType::Decimal
                    )
                )
        })
        .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
}

pub(super) fn bind(
    state: &State,
    parent: usize,
    child: usize,
    declaration: usize,
    relationship: &NativeRelationship,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Link, EditError> {
    let rule = relationship.declaration();
    let width = rule.columns().len();
    if width == 0 || width > INDEX_KEY_COLUMNS {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let mut link = Link {
        parent,
        child,
        declaration,
        parents: [0; INDEX_KEY_COLUMNS],
        children: [0; INDEX_KEY_COLUMNS],
        width,
        action: rule
            .rules()
            .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?
            .on_update,
        matches: Vec::new(),
    };
    let parent = &state.tables[parent];
    let child = &state.tables[child];
    for (position, pair) in rule.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let p = column(parent, pair.parent().as_str(), budget)?;
        let c = column(child, pair.child().as_str(), budget)?;
        let parent_column = &parent.before.schema().columns()[p];
        let child_column = &child.before.schema().columns()[c];
        if parent_column.scalar_type() != child_column.scalar_type()
            || parent_column.decimal_parameters() != child_column.decimal_parameters()
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        link.parents[position] = p;
        link.children[position] = c;
    }
    let mut unique = false;
    for checked in parent.before.indexes() {
        check(budget, cancelled)?;
        let index = checked.index();
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        if index.constraints().is_some_and(|rules| rules.unique())
            && index.columns().len() == width
            && index
                .columns()
                .iter()
                .zip(&link.parents[..width])
                .all(|(key, &position)| {
                    key.number() == parent.before.schema().columns()[position].number()
                })
        {
            unique = true;
        }
    }
    if !unique {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let parent_facts = parent
        .update_facts
        .as_ref()
        .ok_or(InspectError::Malformed)?;
    let child_facts = child.update_facts.as_ref().ok_or(InspectError::Malformed)?;
    let mut keys = Vec::new();
    for row in 0..parent.references.len() {
        check(budget, cancelled)?;
        let key = parent_facts.key(row, &link.parents[..width], budget)?;
        if key.iter().all(Option::is_none) {
            continue;
        }
        reserve(&mut keys, budget)?;
        keys.push((key, row));
    }
    let levels = keys.len().checked_ilog2().unwrap_or(0) as u64 + 1;
    budget
        .charge(Resource::Steps, keys.len() as u64 * levels)
        .map_err(InspectError::from)?;
    keys.sort_unstable();
    budget
        .charge(Resource::Steps, keys.len() as u64)
        .map_err(InspectError::from)?;
    if keys.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    for row in 0..child.references.len() {
        check(budget, cancelled)?;
        let key = child_facts.key(row, &link.children[..width], budget)?;
        if key.iter().all(Option::is_none) {
            continue;
        }
        budget
            .charge(Resource::Steps, levels)
            .map_err(InspectError::from)?;
        let matched = keys
            .binary_search_by_key(&key, |entry| entry.0)
            .map_err(|_| EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
        reserve(&mut link.matches, budget)?;
        link.matches.push((keys[matched].1, row));
    }
    Ok(link)
}
