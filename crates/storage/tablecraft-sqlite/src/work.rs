use crate::{SqliteError, limits::VM_CHECK_INTERVAL};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct State {
    budget: ResourceBudget,
    stopped: Option<SqliteError>,
}

#[derive(Clone)]
pub(crate) struct Work {
    state: Arc<Mutex<State>>,
    cancelled: Arc<AtomicBool>,
}

impl Work {
    pub fn new(budget: ResourceBudget, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                budget,
                stopped: None,
            })),
            cancelled,
        }
    }

    pub fn charge(&self, resource: Resource, amount: u64) -> Result<(), SqliteError> {
        let mut state = self.state.lock().map_err(|_| SqliteError::Database)?;
        if let Some(error) = state.stopped {
            return Err(error);
        }

        let result = if self.cancelled.load(Ordering::Relaxed) {
            Err(SqliteError::Cancelled)
        } else {
            state
                .budget
                .charge(resource, amount)
                .map_err(SqliteError::from)
        };
        if let Err(error) = result {
            state.stopped = Some(error);
        }
        result
    }

    pub fn check(&self) -> Result<(), SqliteError> {
        self.charge(Resource::Steps, 1)
    }

    #[cfg(test)]
    pub fn state_for_test(&self, resource: Resource) -> u64 {
        self.state.lock().unwrap().budget.remaining(resource)
    }

    pub fn install(&self, connection: &rusqlite::Connection) -> Result<(), SqliteError> {
        let work = self.clone();
        connection.progress_handler(
            VM_CHECK_INTERVAL,
            Some(move || {
                work.charge(Resource::Steps, VM_CHECK_INTERVAL as u64)
                    .is_err()
            }),
        )?;
        Ok(())
    }

    pub fn sql<T>(&self, result: rusqlite::Result<T>) -> Result<T, SqliteError> {
        result.map_err(|error| {
            self.state
                .lock()
                .ok()
                .and_then(|state| state.stopped)
                .unwrap_or_else(|| error.into())
        })
    }

    pub fn reserve<T>(&self, values: &mut Vec<T>, additional: usize) -> Result<(), SqliteError> {
        let bytes = additional
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(SqliteError::StructureLimit)?;
        self.charge(Resource::Bytes, bytes as u64)?;
        values
            .try_reserve_exact(additional)
            .map_err(|_| SqliteError::Allocation)
    }

    pub fn text(&self, bytes: &[u8]) -> Result<String, SqliteError> {
        let text = std::str::from_utf8(bytes).map_err(|_| SqliteError::Value)?;
        self.charge(Resource::Bytes, bytes.len() as u64)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| SqliteError::Allocation)?;
        owned.push_str(text);
        Ok(owned)
    }
}
