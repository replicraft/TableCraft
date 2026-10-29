//! Lazy immutable key projections, separate from DELETE association positions.
use super::super::{EditError, facts as source_facts, graph, state::State};
use crate::{
    InspectError, NativeImage, NativeRelationship, TableSnapshot,
    format::layout::INDEX_KEY_COLUMNS, header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

/// Separate immutable projection: existing DELETE links retain their positions.
pub(in crate::table::mutation::delete::cascade) struct Facts {
    snapshot: TableSnapshot,
    positions: Vec<usize>,
}

impl Facts {
    fn position(&self, column: usize, budget: &mut ResourceBudget) -> Result<usize, EditError> {
        budget
            .charge(Resource::Steps, self.positions.len() as u64)
            .map_err(InspectError::from)?;
        self.positions
            .iter()
            .position(|&native| native == column)
            .ok_or(InspectError::Malformed.into())
    }

    pub(in crate::table::mutation::delete::cascade) fn value(
        &self,
        row: usize,
        column: usize,
        budget: &mut ResourceBudget,
    ) -> Result<&Value, EditError> {
        let projected = self.position(column, budget)?;
        self.snapshot
            .rows()
            .get(row)
            .and_then(|values| values.get(projected))
            .ok_or(InspectError::Malformed.into())
    }

    pub(super) fn key(
        &self,
        row: usize,
        columns: &[usize],
        budget: &mut ResourceBudget,
    ) -> Result<graph::Key, EditError> {
        let mut positions = [0; INDEX_KEY_COLUMNS];
        for (position, &column) in columns.iter().enumerate() {
            positions[position] = self.position(column, budget)?;
        }
        let values = self
            .snapshot
            .rows()
            .get(row)
            .ok_or(InspectError::Malformed)?;
        graph::key(values, &positions[..columns.len()])
    }
}

pub(super) fn capture(
    state: &mut State,
    target: usize,
    image: &NativeImage,
    relationships: &[NativeRelationship],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let table = &mut state.tables[target];
    if table.update_facts.is_some() {
        return Ok(());
    }
    let (snapshot, positions) = source_facts::read(
        image,
        table.before.schema(),
        relationships,
        source_facts::Purpose::Update,
        budget,
        cancelled,
    )?;
    if snapshot.schema().definition_page() != table.before.schema().definition_page()
        || snapshot.schema().object_id() != table.before.schema().object_id()
        || snapshot.rows().len() != table.references.len()
    {
        return Err(EditError::Preservation);
    }
    let facts = Facts {
        snapshot,
        positions,
    };
    for (projected, &native) in table.native_positions.iter().enumerate() {
        for (row, values) in table.snapshot.rows().iter().enumerate() {
            check(budget, cancelled)?;
            if values.get(projected) != Some(facts.value(row, native, budget)?) {
                return Err(EditError::Preservation);
            }
        }
    }
    table.update_facts = Some(facts);
    Ok(())
}
