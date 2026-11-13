//! Metadata comparison detects a wrong publication; it supplies no native proof.
#[cfg(test)]
mod tests;
use crate::workspace::objects::memory::Counter;
use crate::{Database, QueryCreationCatalogChange};

const QUERY_STORAGE: &str = "MSysQueries";

#[derive(Clone, Copy)]
pub(super) enum Change<'a> {
    Unchanged,
    QueryCreated {
        object_id: u128,
        name: &'a str,
        storage: QueryCreationCatalogChange<'a>,
    },
    TableDropped(&'a str),
    TableCreated(&'a str),
}

pub(super) fn matches(old: &Database, fresh: &Database) -> bool {
    matches_change(old, fresh, Change::Unchanged)
}

pub(super) fn matches_change(old: &Database, fresh: &Database, change: Change<'_>) -> bool {
    fresh.document_revision > old.document_revision
        && fresh.format == old.format
        && fresh.title_path == old.title_path
        && match change {
            Change::Unchanged
            | Change::QueryCreated {
                storage: QueryCreationCatalogChange::ExistingTables,
                ..
            } => fresh.tables.len() == old.tables.len(),
            Change::QueryCreated {
                storage: QueryCreationCatalogChange::InitializedQueries { storage },
                ..
            } => {
                old.tables.iter().all(|table| table.name != QUERY_STORAGE)
                    && old.tables.len().checked_add(1) == Some(fresh.tables.len())
                    && fresh
                        .tables
                        .iter()
                        .filter(|table| table.name == QUERY_STORAGE)
                        .count()
                        == 1
                    && fresh.tables.iter().any(|table| {
                        table.name == QUERY_STORAGE
                            && table.local
                            && table.system
                            && table.storage == storage
                    })
            }
            Change::TableCreated(name) => {
                old.tables.iter().all(|table| table.name != name)
                    && old.tables.len().checked_add(1) == Some(fresh.tables.len())
                    && fresh
                        .tables
                        .iter()
                        .filter(|table| table.name == name && table.local && !table.system)
                        .count()
                        == 1
                    && fresh
                        .tables
                        .iter()
                        .filter(|table| table.name == name)
                        .count()
                        == 1
            }
            Change::TableDropped(name) => {
                old.tables
                    .iter()
                    .filter(|table| table.name == name && table.local && !table.system)
                    .count()
                    == 1
                    && old.tables.iter().filter(|table| table.name == name).count() == 1
                    && fresh.tables.len().checked_add(1) == Some(old.tables.len())
                    && fresh.tables.iter().all(|table| table.name != name)
            }
        }
        && match change {
            Change::QueryCreated {
                object_id, name, ..
            } => {
                old.queries
                    .iter()
                    .all(|query| query.object_id != object_id && query.name != name)
                    && old.queries.len().checked_add(1) == Some(fresh.queries.len())
                    && fresh
                        .queries
                        .iter()
                        .filter(|query| query.object_id == object_id)
                        .count()
                        == 1
                    && fresh
                        .queries
                        .iter()
                        .filter(|query| query.name == name)
                        .count()
                        == 1
                    && fresh.queries.iter().any(|query| {
                        query.object_id == object_id && query.name == name && !query.system
                    })
            }
            _ => fresh.queries.len() == old.queries.len(),
        }
        && old
            .tables
            .iter()
            .filter(|table| match change {
                Change::Unchanged | Change::QueryCreated { .. } | Change::TableCreated(_) => true,
                Change::TableDropped(name) => table.name != name,
            })
            .zip(fresh.tables.iter().filter(|table| match change {
                Change::TableCreated(name) => table.name != name,
                Change::QueryCreated {
                    storage: QueryCreationCatalogChange::InitializedQueries { .. },
                    ..
                } => table.name != QUERY_STORAGE,
                Change::Unchanged | Change::QueryCreated { .. } | Change::TableDropped(_) => true,
            }))
            .all(|(old, fresh)| {
                old.name == fresh.name
                    && old.local == fresh.local
                    && old.system == fresh.system
                    && old.storage == fresh.storage
            })
        && old
            .queries
            .iter()
            .zip(fresh.queries.iter().filter(|query| match change {
                Change::QueryCreated { object_id, .. } => query.object_id != object_id,
                _ => true,
            }))
            .all(|(old, fresh)| {
                old.object_id == fresh.object_id
                    && old.name == fresh.name
                    && old.system == fresh.system
            })
}

pub(super) fn retained_bytes(database: &Database) -> Option<usize> {
    let mut count = Counter::new(size_of::<Database>());
    count.optional(&database.title_path);
    count.string(&database.title);
    count.string(&database.format);
    count.vector(&database.tables);
    for table in &database.tables {
        count.string(&table.name);
        count.string(&table.storage);
    }
    count.vector(&database.queries);
    for query in &database.queries {
        count.string(&query.name);
    }
    count.finish()
}
