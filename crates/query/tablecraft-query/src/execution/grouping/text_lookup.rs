//! Opt-in symmetric Text hints only prune candidates; retained keys still compare fully.
use crate::{
    QueryError,
    execution::{
        cell::{CellValue, comparison_policy},
        collation::predicate_error,
        exact_keys::directory::Directory,
    },
    source::{CollationResolver, TextCollationId, TextPolicy},
};
use std::{collections::hash_map::DefaultHasher, hash::Hasher};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionValue, Value},
};

pub(super) struct TextLookup {
    pub(super) directory: Directory,
    enabled: bool,
    policy: Option<TextCollationId>,
}

impl Default for TextLookup {
    fn default() -> Self {
        Self {
            directory: Directory::default(),
            enabled: true,
            policy: None,
        }
    }
}

impl TextLookup {
    pub(super) fn fingerprint(
        &mut self,
        cells: &[CellValue],
        fallback: TextPolicy,
        resolver: Option<&dyn CollationResolver>,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Option<u64>, QueryError> {
        if cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(QueryError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        if !self.enabled {
            return Ok(None);
        }
        let result = self.key(cells, fallback, resolver)?;
        if result.is_none() {
            // Retire all hints permanently: every existing representative remains
            // in the retained groups and is visible to the original strict scan.
            self.enabled = false;
            self.directory = Directory::default();
        }
        Ok(result)
    }

    fn key(
        &mut self,
        cells: &[CellValue],
        fallback: TextPolicy,
        resolver: Option<&dyn CollationResolver>,
    ) -> Result<Option<u64>, QueryError> {
        let [cell] = cells else {
            return Ok(None);
        };
        let mut hash = DefaultHasher::new();
        match &cell.value {
            ExpressionValue::Value(Value::Null) => hash.write_u8(0),
            ExpressionValue::Value(Value::Text(text)) => {
                let TextPolicy::Column(policy) = comparison_policy(cell, cell, fallback) else {
                    return Ok(None);
                };
                if self.policy.is_some_and(|previous| previous != policy) {
                    return Ok(None);
                }
                let Some(resolver) = resolver else {
                    return Ok(None);
                };
                let key = resolver
                    .grouping_key(policy, text)
                    .map_err(|error| match error {
                        QueryError::Evaluation(error) => predicate_error(error),
                        error => error,
                    })?;
                let Some(key) = key else {
                    return Ok(None);
                };
                if key.policy() != policy {
                    return Err(QueryError::Source);
                }
                self.policy = Some(policy);
                hash.write_u8(1);
                hash.write(key.as_bytes());
            }
            _ => return Ok(None),
        }
        Ok(Some(hash.finish()))
    }
}
