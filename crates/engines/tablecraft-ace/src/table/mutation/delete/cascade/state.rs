//! Bounded table discovery, checked explicit seeds and monotone original-root closure.
use super::{EditError, capture, facts, graph, reserve};
use crate::{
    InspectError, NativeImage, NativeRelationship, TableCheck, TableSnapshot, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    inspect_relationships,
    table::mutation::{
        delete::{TableRowDeletes, patches},
        gates, rows,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ReferentialAction;

pub(super) struct Table {
    pub before: TableCheck,
    pub snapshot: TableSnapshot,
    /// Projected row position to the complete native schema's vector position.
    pub native_positions: Vec<usize>,
    pub references: Vec<u32>,
    pub selected: Vec<bool>,
}

pub(super) struct State {
    pub tables: Vec<Table>,
    links: Vec<graph::Link>,
    relationships: Vec<NativeRelationship>,
    queue: Vec<(usize, usize)>,
}

impl State {
    pub fn prepare(
        image: &NativeImage,
        groups: &[TableRowDeletes<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        if groups.len() > NativeImage::MAX_DELETE_BATCH_TABLES {
            return Err(InspectError::StructureLimit.into());
        }
        let relationships = inspect_relationships(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            budget,
            cancelled,
        )?;
        let mut state = Self {
            tables: Vec::new(),
            links: Vec::new(),
            relationships: Vec::new(),
            queue: Vec::new(),
        };
        let mut seeds = 0_usize;
        for (position, group) in groups.iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, position as u64)
                .map_err(InspectError::from)?;
            if groups[..position]
                .iter()
                .any(|prior| prior.table().eq_ignore_ascii_case(group.table()))
            {
                return Err(EditError::Conflict);
            }
            seeds = seeds
                .checked_add(group.deletes().len())
                .filter(|count| *count <= NativeImage::MAX_DELETE_BATCH_ROWS)
                .ok_or(InspectError::StructureLimit)?;
            let table = state.load(image, group.table(), &relationships, budget, cancelled)?;
            rows::unique(group.deletes(), budget, cancelled)?;
            for request in group.deletes() {
                check(budget, cancelled)?;
                if request.expected_bytes().len() > PAGE_BYTES {
                    return Err(InspectError::StructureLimit.into());
                }
                budget
                    .charge(Resource::Steps, request.expected_bytes().len() as u64)
                    .map_err(InspectError::from)?;
            }
            let mut source = image.reader();
            let mut reader = Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            };
            let (_, references) = patches::records(
                &mut reader,
                state.tables[table].before.schema(),
                group.deletes(),
            )?;
            // patches::records visits sequentially. Map its checked physical roots
            // back to original ordinals; request order never becomes row identity.
            for reference in references {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, state.tables[table].references.len() as u64)
                    .map_err(InspectError::from)?;
                let row = state.tables[table]
                    .references
                    .iter()
                    .position(|value| *value == reference)
                    .ok_or(InspectError::Malformed)?;
                state.select(table, row, budget)?;
            }
        }
        state.relationships = relationships;
        Ok(state)
    }

    fn load(
        &mut self,
        image: &NativeImage,
        name: &str,
        relationships: &[NativeRelationship],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.tables.len() as u64)
            .map_err(InspectError::from)?;
        if let Some(position) = self
            .tables
            .iter()
            .position(|table| table.before.schema().name().eq_ignore_ascii_case(name))
        {
            return Ok(position);
        }
        if self.tables.len() == NativeImage::MAX_DELETE_BATCH_TABLES {
            return Err(InspectError::StructureLimit.into());
        }
        let before = gates::before_delete(image, name, budget, cancelled)?;
        if self.tables.iter().any(|table| {
            table.before.schema().definition_page() == before.schema().definition_page()
                || table.before.schema().object_id() == before.schema().object_id()
        }) {
            return Err(EditError::Conflict);
        }
        // Complete structural/field/relationship gates above remain mandatory.
        // Only this redundant association-facts pass projects unrelated values.
        let (snapshot, native_positions) =
            facts::read(image, before.schema(), relationships, budget, cancelled)?;
        if before.schema().definition_page() != snapshot.schema().definition_page()
            || before.schema().object_id() != snapshot.schema().object_id()
            || before.rows() != snapshot.rows().len()
        {
            return Err(InspectError::Malformed.into());
        }
        let references = capture::references(image, before.schema(), budget, cancelled)?;
        if references.len() != snapshot.rows().len() {
            return Err(InspectError::Malformed.into());
        }
        let mut selected = Vec::new();
        budget
            .charge_allocation(references.len() as u64, size_of::<bool>() as u64)
            .map_err(InspectError::from)?;
        selected
            .try_reserve_exact(references.len())
            .map_err(|_| InspectError::Allocation)?;
        selected.resize(references.len(), false);
        reserve(&mut self.tables, budget)?;
        self.tables.push(Table {
            before,
            snapshot,
            native_positions,
            references,
            selected,
        });
        Ok(self.tables.len() - 1)
    }

    pub fn expand(
        &mut self,
        image: &NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let relationships = std::mem::take(&mut self.relationships);
        let mut parent = 0;
        while parent < self.tables.len() {
            for relationship in &relationships {
                check(budget, cancelled)?;
                let declaration = relationship.declaration();
                let name = self.tables[parent].before.schema().name();
                let outgoing = declaration
                    .parent_table()
                    .as_str()
                    .eq_ignore_ascii_case(name);
                let incoming = declaration
                    .child_table()
                    .as_str()
                    .eq_ignore_ascii_case(name);
                if !outgoing && !incoming {
                    continue;
                }
                let rules = declaration
                    .rules()
                    .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
                if !rules.enforced
                    || !matches!(
                        rules.on_update,
                        ReferentialAction::Restrict | ReferentialAction::Cascade
                    )
                    || !matches!(
                        rules.on_delete,
                        ReferentialAction::Restrict | ReferentialAction::Cascade
                    )
                    || declaration
                        .parent_table()
                        .as_str()
                        .eq_ignore_ascii_case(declaration.child_table().as_str())
                {
                    return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
                }
                if outgoing && rules.on_delete == ReferentialAction::Cascade {
                    let child = self.load(
                        image,
                        declaration.child_table().as_str(),
                        &relationships,
                        budget,
                        cancelled,
                    )?;
                    let link =
                        graph::link(&self.tables, parent, child, relationship, budget, cancelled)?;
                    reserve(&mut self.links, budget)?;
                    self.links.push(link);
                }
            }
            parent += 1;
        }
        graph::acyclic(self.tables.len(), &self.links, budget, cancelled)
    }

    fn select(
        &mut self,
        table: usize,
        row: usize,
        budget: &mut ResourceBudget,
    ) -> Result<(), EditError> {
        if !self.tables[table].selected[row] {
            if self.queue.len() == NativeImage::MAX_DELETE_BATCH_ROWS {
                return Err(InspectError::StructureLimit.into());
            }
            reserve(&mut self.queue, budget)?;
            self.queue.push((table, row));
            self.tables[table].selected[row] = true;
        }
        Ok(())
    }

    pub fn close(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let mut next = 0;
        while next < self.queue.len() {
            check(budget, cancelled)?;
            let (table, row) = self.queue[next];
            for link in 0..self.links.len() {
                check(budget, cancelled)?;
                if self.links[link].parent != table {
                    continue;
                }
                for pair in 0..self.links[link].matches.len() {
                    check(budget, cancelled)?;
                    let (parent, child) = self.links[link].matches[pair];
                    if parent == row {
                        self.select(self.links[link].child, child, budget)?;
                    }
                }
            }
            next += 1;
        }
        Ok(())
    }
}
