//! Expand proved retained lists without reopening their original source.
use super::{CollectionFrame, JoinedFrame, MISSING_LIST, Requests, Runtime};
use crate::{
    OutputType, QueryError,
    execution::cell::CellValue,
    plan::{CollectionOrigin, ReadPlan},
    source::MultivalueList,
};
use tablecraft_core::limits::Resource;
use tablecraft_schema::Column;

impl Runtime<'_, '_> {
    pub(super) fn retained_collection_relation(
        &mut self,
        read: &ReadPlan,
        expansion: &[usize],
        requests: &Requests<'_>,
        base: usize,
        leaf: usize,
    ) -> Result<Vec<JoinedFrame>, QueryError> {
        let input = self.collection_read(read)?;
        let needs_criteria = if let Some(parent) = requests.parent {
            let mut found = false;
            for location in parent.locations.as_ref().ok_or(QueryError::Source)? {
                self.tick()?;
                found |= location.leaf == leaf;
            }
            found
        } else {
            false
        };
        let mut scalar_width = 0;
        let mut list_width = 0;
        if !expansion.is_empty() || needs_criteria {
            for kind in &read.output_types {
                self.tick()?;
                if kind.is_multivalue() {
                    list_width += 1;
                } else {
                    scalar_width += 1;
                }
            }
        }
        let schema = if expansion.is_empty() {
            None
        } else {
            Some(self.retained_collection_schema(read)?)
        };
        let layout = schema
            .as_ref()
            .map(|(columns, origins)| {
                self.retained_collection_expansion(columns, origins, expansion)
            })
            .transpose()?;
        let mut result = Vec::new();
        for mut frame in input {
            self.tick()?;
            if (!expansion.is_empty() || needs_criteria)
                && (frame.scalars.len() != scalar_width || frame.lists.len() != list_width)
            {
                return Err(QueryError::Width);
            }
            if let Some(layout) = &layout {
                let views = self.retained_collection_views(&frame, expansion)?;
                let mut rows = Vec::new();
                self.expand_collection_row(&frame.scalars, layout, &views, &mut rows)?;
                for scalars in rows {
                    let row = self.copy_retained_relation_frame(
                        &frame,
                        scalars,
                        requests,
                        leaf,
                        needs_criteria,
                    )?;
                    self.push(&mut result, row)?;
                }
            } else if needs_criteria {
                let scalars = std::mem::take(&mut frame.scalars);
                let row =
                    self.copy_retained_relation_frame(&frame, scalars, requests, leaf, true)?;
                self.push(&mut result, row)?;
            } else {
                // The existing no-expansion/no-predicate path moves its lists.
                self.push(
                    &mut result,
                    JoinedFrame {
                        frame,
                        criteria: Vec::new(),
                    },
                )?;
            }
        }
        for row in &mut result {
            for (index, _) in &mut row.frame.lists {
                self.tick()?;
                *index = index.checked_add(base).ok_or(QueryError::StructureLimit)?;
            }
        }
        Ok(result)
    }

    fn retained_collection_schema(
        &mut self,
        read: &ReadPlan,
    ) -> Result<(Vec<Column>, Vec<CollectionOrigin>), QueryError> {
        let count = read
            .output_types
            .iter()
            .filter(|kind| kind.is_multivalue())
            .count();
        self.budget.charge_allocation(
            count as u64,
            (size_of::<Column>() + size_of::<CollectionOrigin>()) as u64,
        )?;
        let mut columns = Vec::new();
        let mut origins = Vec::new();
        columns
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        origins
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        for (output, kind) in read.output_types.iter().enumerate() {
            self.tick()?;
            if let OutputType::Multivalue {
                element_type,
                element_nullable,
                ..
            } = kind
            {
                let name = &read.names[output];
                self.budget.charge(Resource::Bytes, name.len() as u64)?;
                let mut copied = String::new();
                copied
                    .try_reserve_exact(name.len())
                    .map_err(|_| QueryError::Allocation)?;
                copied.push_str(name);
                columns.push(Column::with_type(copied, *element_type, *element_nullable)?);
                origins.push(
                    read.collection_origin(output)
                        .ok_or(QueryError::Unsupported)?,
                );
            }
        }
        Ok((columns, origins))
    }

    fn retained_collection_views<'a>(
        &mut self,
        frame: &'a CollectionFrame,
        expansion: &[usize],
    ) -> Result<Vec<&'a dyn MultivalueList>, QueryError> {
        self.budget.charge_allocation(
            expansion.len() as u64,
            size_of::<&dyn MultivalueList>() as u64,
        )?;
        let mut views = Vec::new();
        views
            .try_reserve_exact(expansion.len())
            .map_err(|_| QueryError::Allocation)?;
        for index in expansion {
            let mut found = None;
            for (slot, value) in &frame.lists {
                self.tick()?;
                if slot == index {
                    found = Some(value.as_ref());
                    break;
                }
            }
            views.push(match found.ok_or(QueryError::Source)? {
                Some(value) => &value.values as &dyn MultivalueList,
                None => &MISSING_LIST,
            });
        }
        Ok(views)
    }

    fn copy_retained_relation_frame(
        &mut self,
        frame: &CollectionFrame,
        scalars: Vec<CellValue>,
        requests: &Requests<'_>,
        leaf: usize,
        needs_criteria: bool,
    ) -> Result<JoinedFrame, QueryError> {
        let mut lists = Vec::new();
        for (index, value) in &frame.lists {
            self.tick()?;
            let value = self.copy_collection_presence(value.as_ref())?;
            self.push(&mut lists, (*index, value))?;
        }
        let mut criteria = Vec::new();
        if needs_criteria {
            let parent = requests.parent.ok_or(QueryError::Source)?;
            for (position, location) in parent
                .locations
                .as_ref()
                .ok_or(QueryError::Source)?
                .iter()
                .enumerate()
            {
                self.tick()?;
                if location.leaf != leaf {
                    continue;
                }
                let mut found = None;
                for (index, value) in &frame.lists {
                    self.tick()?;
                    if *index == location.member {
                        found = Some(value.as_ref());
                        break;
                    }
                }
                let value = match found.ok_or(QueryError::Source)? {
                    Some(value) => Some(self.copy_joined_criterion(&value.values)?),
                    None => None,
                };
                self.push(&mut criteria, (position, value))?;
            }
        }
        Ok(JoinedFrame {
            frame: CollectionFrame {
                scalars,
                lists,
                keys: Vec::new(),
            },
            criteria,
        })
    }
}
