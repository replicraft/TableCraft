//! Missing parents are explicit sidecar absence, never a stored child value.
use super::{CollectionFrame, JoinedFrame, Requests, Runtime};
use crate::{QueryError, plan::Relation};
use std::ops::Range;

impl Runtime<'_, '_> {
    pub(super) fn missing_collection_side(
        &mut self,
        relation: &Relation,
        requests: &Requests<'_>,
        base: usize,
        leaves: Range<usize>,
    ) -> Result<JoinedFrame, QueryError> {
        let mut lists = Vec::new();
        for slot in &relation.slots {
            self.tick()?;
            if let Some(local) = slot.collection_index {
                let index = base.checked_add(local).ok_or(QueryError::StructureLimit)?;
                for selected in requests.selected {
                    self.tick()?;
                    if *selected == index {
                        self.push(&mut lists, (index, None))?;
                        break;
                    }
                }
            }
        }
        let mut criteria = Vec::new();
        if let Some(parent) = requests.parent {
            for (position, location) in parent
                .locations
                .as_ref()
                .ok_or(QueryError::Source)?
                .iter()
                .enumerate()
            {
                self.tick()?;
                if leaves.contains(&location.leaf) {
                    self.push(&mut criteria, (position, None))?;
                }
            }
        }
        Ok(JoinedFrame {
            frame: CollectionFrame {
                // join_missing supplies exactly the missing scalar width. This
                // frame carries only collection presence for the missing side.
                scalars: Vec::new(),
                lists,
                keys: Vec::new(),
            },
            criteria,
        })
    }
}
