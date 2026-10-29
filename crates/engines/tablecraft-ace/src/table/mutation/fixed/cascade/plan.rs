use super::super::super::{Ace16WritePreview, EditError, WriteBlocker, preview::qualify};
use super::super::{FixedColumnUpdates, FixedScalarUpdate};
use super::cells::{Cell, Cells, reserve, scalar};
use super::graph;
use crate::{InspectError, NativeImage, header::check, inspect_relationships};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Group {
    table: usize,
    column: usize,
    updates: Vec<FixedScalarUpdate>,
}
impl NativeImage {
    /// Prepare an ACE 16 engineering candidate with declared update cascades.
    ///
    /// Original integral/Currency/GUID/Decimal parent-child matches are frozen before any
    /// assignment. Decimal pairs require identical native precision and scale.
    /// Explicit cells pin final values, including no-ops; conflicting
    /// propagated demands fail. RESTRICT rejects changing a referenced original
    /// key, even if a final key permutation would satisfy membership alone.
    /// Multi-hop/cyclic propagation reaches a bounded fixed point before final
    /// complete-key validation and the ordinary batch preservation/index gates.
    /// Delete/SET NULL actions, SQL execution and production saves are separate.
    pub fn preview_ace16_cascading_updates(
        self,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span =
            tracing::debug_span!("native.edit.cascade_preview", groups = groups.len()).entered();
        check(budget, cancelled)?;
        qualify(&self)?;
        let mut cells = Cells {
            tables: Vec::new(),
            facts: Vec::new(),
        };
        explicit(&mut cells, &self, groups, budget, cancelled)?;
        if cells.facts.is_empty() {
            return self.preview_fixed_batch_inner::<false>(groups, budget, cancelled);
        }
        let declarations = inspect_relationships(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            budget,
            cancelled,
        )?;
        let mut links = Vec::new();
        let mut parent = 0;
        while parent < cells.tables.len() {
            check(budget, cancelled)?;
            for declaration in &declarations {
                check(budget, cancelled)?;
                if !declaration
                    .declaration()
                    .parent_table()
                    .as_str()
                    .eq_ignore_ascii_case(cells.tables[parent].schema().name())
                {
                    continue;
                }
                let rules = declaration
                    .declaration()
                    .rules()
                    .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
                if !rules.enforced {
                    continue;
                }
                let link = graph::link(&mut cells, &self, parent, declaration, budget, cancelled)?;
                reserve(&mut links, budget)?;
                links.push(link);
            }
            parent += 1;
        }
        loop {
            check(budget, cancelled)?;
            let mut changed = false;
            for link in &links {
                changed |= link.propagate(&mut cells, false, budget, cancelled)?;
            }
            if !changed {
                break;
            }
        }
        for link in &links {
            link.propagate(&mut cells, true, budget, cancelled)?;
        }
        let groups = output(&cells, groups, budget, cancelled)?;
        let mut borrowed = Vec::new();
        for group in &groups {
            check(budget, cancelled)?;
            reserve(&mut borrowed, budget)?;
            borrowed.push(FixedColumnUpdates::new(
                cells.tables[group.table].schema().name(),
                cells.tables[group.table].columns()[group.column].name(),
                &group.updates,
            ));
        }
        self.preview_fixed_batch_inner::<false>(&borrowed, budget, cancelled)
    }
}
fn explicit(
    cells: &mut Cells,
    image: &NativeImage,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    let mut count = 0_usize;
    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        count = count
            .checked_add(group.updates().len())
            .filter(|c| *c <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if groups[..position].iter().any(|g| {
            g.table().eq_ignore_ascii_case(group.table())
                && g.column().eq_ignore_ascii_case(group.column())
        }) {
            return Err(EditError::Conflict);
        }
        let table = graph::table(cells, image, group.table(), budget, cancelled)?;
        let column = cells.tables[table]
            .columns()
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(group.column()))
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        for update in group.updates() {
            check(budget, cancelled)?;
            let cell = Cell {
                table,
                column,
                row: update.row(),
            };
            if cells.original(cell)? != update.expected() {
                return Err(EditError::Conflict);
            }
            cells.assign(cell, scalar(update.value())?, true, budget, cancelled)?;
        }
    }
    Ok(())
}
fn output(
    cells: &Cells,
    requested: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Group>, EditError> {
    let mut groups: Vec<Group> = Vec::new();
    // Keep empty requested groups in the final batch as well: they still name
    // tables/fields whose normal gates must run, just as in explicit batches.
    for group in requested {
        check(budget, cancelled)?;
        let table = cells
            .tables
            .iter()
            .position(|t| t.schema().name().eq_ignore_ascii_case(group.table()))
            .ok_or(InspectError::Malformed)?;
        let column = cells.tables[table]
            .columns()
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(group.column()))
            .ok_or(InspectError::Malformed)?;
        budget
            .charge(
                Resource::Steps,
                (cells.tables.len() + cells.tables[table].columns().len()) as u64,
            )
            .map_err(InspectError::from)?;
        reserve(&mut groups, budget)?;
        groups.push(Group {
            table,
            column,
            updates: Vec::new(),
        });
    }
    for fact in &cells.facts {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, groups.len() as u64)
            .map_err(InspectError::from)?;
        let position = match groups
            .iter()
            .position(|g| g.table == fact.cell.table && g.column == fact.cell.column)
        {
            Some(position) => position,
            None => {
                if groups.len() >= NativeImage::MAX_FIXED_BATCH_GROUPS {
                    return Err(InspectError::StructureLimit.into());
                }
                reserve(&mut groups, budget)?;
                groups.push(Group {
                    table: fact.cell.table,
                    column: fact.cell.column,
                    updates: Vec::new(),
                });
                groups.len() - 1
            }
        };
        let update = FixedScalarUpdate::new(
            fact.cell.row,
            scalar(cells.original(fact.cell)?)?,
            scalar(&fact.value)?,
        );
        reserve(&mut groups[position].updates, budget)?;
        groups[position].updates.push(update);
    }
    Ok(groups)
}
